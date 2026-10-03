use std::{collections::HashMap, error::Error};

use rusqlite::{Connection, params_from_iter};
use tiny_http::{Request, Response, Server, StatusCode};

#[cfg(feature = "altcha")]
mod altcha;

fn main() {
    let token =
        std::env::var("MEMBER_MGR_TOKEN").unwrap_or_else(|_| "very-secret-token".to_string());
    let use_token = std::env::var("MEMBER_MGR_USE_TOKEN").unwrap_or_else(|_| "no".to_string());
    let addr = std::env::var("MEMBER_MGR_ADDR").unwrap_or_else(|_| "0.0.0.0:1234".to_string());
    let db = std::env::var("MEMBER_MGR_DEFAULT_DB").unwrap_or_else(|_| "members.db".to_string());
    let log = std::env::var("MEMBER_MGR_LOG").unwrap_or_else(|_| "info".to_string());
    let create = std::env::var("MEMBER_MGR_CREATE_DB").unwrap_or_else(|_| "yes".to_string());

    let server = Server::http(&addr).unwrap();

    let log_info = log == "info";
    let create_db = create == "yes";
    let use_token = use_token == "yes";

    if log_info {
        println!("[INFO] Listening on {addr}");
    }

    #[cfg(feature = "altcha")]
    let altcha_state = crate::altcha::get_state();

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eprintln!("[ERR!] recv: {e}");
                continue;
            }
        };

        #[cfg(feature = "altcha")]
        let result = handle_request(
            request,
            &token,
            &db,
            use_token,
            log_info,
            create_db,
            &altcha_state,
        );

        #[cfg(not(feature = "altcha"))]
        let result = handle_request(request, &token, &db, use_token, log_info, create_db);

        if let Err(e) = result {
            eprintln!("[ERR!] request: {e}");
        }
    }
}

#[derive(Debug)]
pub struct SubmitErr(u16, String, String);

impl SubmitErr {
    pub fn new(code: u16, internal_message: impl Into<String>, message: impl Into<String>) -> Self {
        Self(code, internal_message.into(), message.into())
    }
}

impl std::fmt::Display for SubmitErr {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.1)
    }
}

impl Error for SubmitErr {}

fn handle_request(
    mut request: Request,
    token: &str,
    db: &str,
    use_token: bool,
    log_info: bool,
    create_db: bool,

    #[cfg(feature = "altcha")] altcha_state: &altcha::AltchaState,
) -> Result<(), Box<dyn Error>> {
    let authorized = !use_token
        || request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"))
            .and_then(|header| header.value.as_str().strip_prefix("Bearer "))
            .is_some_and(|received| received == token);

    if !authorized {
        request.respond(Response::from_string("Unauthorized").with_status_code(StatusCode(401)))?;

        return Err(
            SubmitErr::new(401, "Unauthorized".to_string(), "Unauthorized".to_string()).into(),
        );
    }

    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;

    let path = request
        .url()
        .split_once('?')
        .map(|(path, _query)| path)
        .unwrap_or_else(|| request.url())
        .to_owned();

    match path.as_str() {
        #[cfg(feature = "altcha")]
        "/challenge" => {
            crate::altcha::get_challenge(request, altcha_state)?;
            Ok(())
        }
        "/submit" => {
            let mut fields: HashMap<String, String> = match serde_urlencoded::from_str(&body) {
                Ok(fields) => fields,
                Err(err) => {
                    request.respond(
                        Response::from_string("Invalid form data".to_string())
                            .with_status_code(StatusCode(400)),
                    )?;
                    return Err(SubmitErr::new(
                        400,
                        format!("Invalid form data: {err}"),
                        "Invalid form data".to_string(),
                    )
                    .into());
                }
            };

            if log_info {
                println!("[INFO] received: {fields:?}");
            }

            #[cfg(feature = "altcha")]
            if let Err(e) = crate::altcha::post_submit(altcha_state, &mut fields) {
                request.respond(Response::from_string(&e.2).with_status_code(StatusCode(e.0)))?;
                return Err(e.into());
            };

            let db_name = fields.remove("db_name");
            if fields.is_empty() {
                if log_info {
                    println!("[INFO] received no data");
                }
            } else {
                insert_sql(&db_name.unwrap_or(db.to_string()), &fields, create_db)?;

                if log_info {
                    println!("[INFO] wrote: {fields:?}");
                }

                request.respond(Response::empty(StatusCode(200)))?;
            }

            Ok(())
        }
        _ => {
            request
                .respond(Response::from_string("Not Found").with_status_code(StatusCode(404)))?;

            Err(SubmitErr::new(404, format!("Not Found: {path}"), "Not found".to_string()).into())
        }
    }
}

fn insert_sql(
    db: &str,
    data: &HashMap<String, String>,
    create_db: bool,
) -> Result<usize, rusqlite::Error> {
    let db = Connection::open(db).unwrap();

    let mut columns = Vec::with_capacity(data.len());
    let mut placeholders = Vec::with_capacity(data.len());
    let mut values = Vec::with_capacity(data.len());

    for (index, (key, value)) in data.iter().enumerate() {
        columns.push(key.as_str());
        placeholders.push(format!("?{}", index + 1));
        values.push(value);
    }

    if create_db {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS members ({})",
            columns.join(", ")
        );
        db.execute(&sql, [])?;
    }

    let sql = format!(
        "INSERT INTO members ({}) VALUES ({})",
        columns.join(", "),
        placeholders.join(", "),
    );

    db.execute(&sql, params_from_iter(values))
}
