use std::{collections::HashMap, error::Error};

use rusqlite::{Connection, params_from_iter};
use tiny_http::{Request, Response, Server, StatusCode};

fn main() {
    let token = std::env::var("MEMBER_MGR_TOKEN").unwrap_or_else(|_| "very-secret-token".to_string());    
    let addr = std::env::var("MEMBER_MGR_ADDR").unwrap_or_else(|_| "0.0.0.0:1234".to_string());
    let db = std::env::var("MEMBER_MGR_DEFAULT_DB").unwrap_or_else(|_| "members.db".to_string());
    let log = std::env::var("MEMBER_MGR_LOG").unwrap_or_else(|_| "info".to_string());
    let create = std::env::var("MEMBER_MGR_CREATE_DB").unwrap_or_else(|_| "yes".to_string());

    let server = Server::http(&addr).unwrap();

    let log_info = log == "info";
    let create_db = create == "yes";

    if log_info {
        println!("[INFO] Listening on {addr}");
    }

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eprintln!("[ERR!] recv: {e}");
                continue;
            }
        };

        if let Err(e) = handle_request(request, &token, &db, log_info, create_db) {
            eprintln!("[ERR!] request: {e}");
        }
    }
}

#[derive(Debug)]
struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "unauthorized")
    }
}

impl Error for Unauthorized {}

fn handle_request(
    mut request: Request,
    token: &str,
    db: &str,
    log_info: bool,
    create_db: bool,
) -> Result<(), Box<dyn Error>> {
    let authorized = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .and_then(|header| header.value.as_str().strip_prefix("Bearer "))
        .is_some_and(|received| received == token);

    if !authorized {
        request.respond(Response::from_string("Unauthorized").with_status_code(StatusCode(401)))?;

        return Err(Unauthorized.into());
    }

    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;

    let mut data = parse_request(&body);

    if log_info {
        println!("[INFO] received: {data:?}");
    }

    let db_name = data.remove("db_name");
    insert_sql(db_name.unwrap_or(db), &data, create_db)?;

    if log_info {
        println!("[INFO] wrote: {data:?}");
    }

    request.respond(Response::empty(StatusCode(200)))?;

    Ok(())
}

fn parse_request(body: &str) -> HashMap<&str, &str> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .collect()
}

fn insert_sql(
    db: &str,
    data: &HashMap<&str, &str>,
    create_db: bool,
) -> Result<usize, rusqlite::Error> {
    let db = Connection::open(db).unwrap();

    let mut columns = Vec::with_capacity(data.len());
    let mut placeholders = Vec::with_capacity(data.len());
    let mut values = Vec::with_capacity(data.len());

    for (index, (key, value)) in data.iter().enumerate() {
        columns.push(*key);
        placeholders.push(format!("?{}", index + 1));
        values.push(*value);
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
