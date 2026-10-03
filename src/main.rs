use std::{collections::HashMap, error::Error};

use rusqlite::{Connection, params_from_iter};
use tiny_http::{Request, Response, Server, StatusCode};

use crate::mail::MailConfig;

#[cfg(feature = "altcha")]
mod altcha;

#[cfg(feature = "mail")]
mod mail;

pub struct Config {
    token: String,
    use_token: bool,
    addr: String,
    db: String,
    log_info: bool,
    create_db: bool,
    #[cfg(feature = "altcha")]
    hmac_secret: String,
    #[cfg(feature = "altcha")]
    hmac_key_secret: String,
    #[cfg(feature = "mail")]
    mail_configs: HashMap<String, MailConfig>
}

impl Config {
    pub fn new() -> Self {
        use std::env::var;
        Self {
            token: var("MEMBER_MGR_TOKEN").unwrap_or_else(|_| "very-secret-token".to_string()),
            use_token: var("MEMBER_MGR_USE_TOKEN").unwrap_or_else(|_| "no".to_string()) == "yes",
            addr: var("MEMBER_MGR_ADDR").unwrap_or_else(|_| "0.0.0.0:1234".to_string()),
            db: var("MEMBER_MGR_DEFAULT_DB").unwrap_or_else(|_| "members.db".to_string()),
            log_info: std::env::var("MEMBER_MGR_LOG").unwrap_or_else(|_| "info".to_string())
                == "info",
            create_db: std::env::var("MEMBER_MGR_CREATE_DB").unwrap_or_else(|_| "yes".to_string())
                == "yes",

            hmac_secret: std::env::var("MEMBER_MGR_ALTCHA_HMAC_SECRET")
                .expect("MEMBER_MGR_ALTCHA_HMAC_SECRET must be set"),
            hmac_key_secret: std::env::var("MEMBER_MGR_ALTCHA_HMAC_KEY_SECRET")
                .expect("MEMBER_MGR_ALTCHA_HMAC_KEY_SECRET must be set"),

            from: std::env::var("MEMBER_MGR_FROM").expect("MEMBER_MGR_FROM must be set"),
            from_name: std::env::var("MEMBER_MGR_FROM_NAME")
                .expect("MEMBER_MGR_FROM_NAME must be set"),
            subject: std::env::var("MEMBER_MGR_SUBJECT").expect("MEMBER_MGR_SUBJECT must be set"),
            body: std::env::var("MEMBER_MGR_BODY").expect("MEMBER_MGR_BODY must be set"),
            smtp_username: std::env::var("MEMBER_MGR_SMTP_USERNAME")
                .expect("MEMBER_MGR_SMTP_USERNAME must be set"),
            smtp_password: std::env::var("MEMBER_MGR_SMTP_PASSWORD")
                .expect("MEMBER_MGR_SMTP_PASSWORD must be set"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

fn main() {
    #[cfg(feature = "dotenvy")]
    dotenvy::dotenv().ok();

    let config = Config::new();

    let server = Server::http(&config.addr).unwrap();

    if config.log_info {
        println!("[INFO] Listening on {}", config.addr);
    }

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eprintln!("[ERR!] recv: {e}");
                continue;
            }
        };

        if let Err(e) = handle_request(request, &config) {
            eprintln!("[ERR!] request: {e}");
        };
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

fn handle_request(mut request: Request, config: &Config) -> Result<(), Box<dyn Error>> {
    let authorized = !config.use_token
        || request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"))
            .and_then(|header| header.value.as_str().strip_prefix("Bearer "))
            .is_some_and(|received| received == config.token);

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
            crate::altcha::get_challenge(request, config)?;
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

            if config.log_info {
                println!("[INFO] received: {fields:?}");
            }

            #[cfg(feature = "altcha")]
            if let Err(e) = crate::altcha::post_submit(config, &mut fields) {
                request.respond(Response::from_string(&e.2).with_status_code(StatusCode(e.0)))?;
                return Err(e.into());
            };

            let db_name = fields.remove("db_name");
            if fields.is_empty() {
                if config.log_info {
                    println!("[INFO] received no data");
                }
            } else {
                insert_sql(
                    &db_name.unwrap_or(config.db.to_string()),
                    &fields,
                    config.create_db,
                )?;

                if config.log_info {
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

    // TODO:
    // #[cfg(feature = "mail")]
    // send_mail();

    db.execute(&sql, params_from_iter(values))
}
