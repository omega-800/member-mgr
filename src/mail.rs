//! Sends plain-text mails for submissions containing an `email` field.
//!
//! Mailing is configured via `MAIL_CFG_<NAME>` environment variables, where `<NAME>`
//! is either `default` or a lowercased sqlite database name to override the default
//! for submissions written to that database. The value is a `;`-separated list of
//! `key=value` pairs, e.g.:
//!
//! ```sh
//! MAIL_CFG_DEFAULT='from=you@example.com;subject=Welcome {{name}};body_path=./welcome.txt'
//! ```
//!
//! Valid keys: `from` (required), `from_name`, `subject` (required), `body_path`
//! (required), `smtp_host` (default `smtp.gmail.com`), `smtp_port` (default 465;
//! 587 selects STARTTLS), `smtp_username` and `smtp_password` (must be set together).
//!
//! Optionally, `additional_receiver`, `additional_subject` and
//! `additional_body_path` (must be set together) add a second mail that is always
//! sent to the same fixed recipient, e.g. an administrative notification. It uses
//! the same sender and SMTP settings as the primary mail.

use std::{collections::HashMap, error::Error, fs};

use lettre::{
    Message, SmtpTransport, Transport,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use crate::Config;

/// Parsed configuration of one `MAIL_CFG_*` environment variable.
pub struct MailConfig {
    from: String,
    from_name: Option<String>,
    subject: String,
    body_path: String,
    smtp_host: String,
    smtp_port: Option<u16>,
    smtp_username: Option<String>,
    smtp_password: Option<String>,
    additional_receiver: Option<String>,
    additional_subject: Option<String>,
    additional_body_path: Option<String>,
}

impl MailConfig {
    /// Parses a `MAIL_CFG_*` value: a `;`-separated list of `key=value` pairs.
    ///
    /// # Errors
    ///
    /// Fails if a field is not `key=value`, `smtp_port` is not a valid port,
    /// `smtp_username`/`smtp_password` are set without each other, the
    /// `additional_*` fields are only partially set, or a required field
    /// (`from`, `subject`, `body_path`) is missing.
    pub fn parse(value: &str) -> Result<Self, String> {
        let fields: HashMap<String, String> = value
            .split(';')
            .filter(|part| !part.trim().is_empty())
            .map(|part| {
                let (key, val) = part
                    .split_once('=')
                    .ok_or_else(|| format!("invalid field '{part}', expected key=value"))?;
                Ok((key.trim().to_owned(), val.trim().to_owned()))
            })
            .collect::<Result<_, String>>()?;

        let get = |name: &str| fields.get(name).map(String::as_str).filter(|v| !v.is_empty());

        let from = get("from").ok_or("missing 'from'")?.to_owned();
        let from_name = get("from_name").map(str::to_owned);
        let subject = get("subject").ok_or("missing 'subject'")?.to_owned();
        let body_path = get("body_path").ok_or("missing 'body_path'")?.to_owned();
        let smtp_host = get("smtp_host").unwrap_or("smtp.gmail.com").to_owned();
        let smtp_port = match get("smtp_port") {
            None => None,
            Some(port) => Some(port.parse::<u16>().map_err(|_| format!("invalid smtp_port '{port}'"))?),
        };
        let smtp_username = get("smtp_username").map(str::to_owned);
        let smtp_password = get("smtp_password").map(str::to_owned);
        let additional_receiver = get("additional_receiver").map(str::to_owned);
        let additional_subject = get("additional_subject").map(str::to_owned);
        let additional_body_path = get("additional_body_path").map(str::to_owned);

        if smtp_username.is_some() != smtp_password.is_some() {
            return Err("smtp_username and smtp_password must be set together".to_string());
        }

        if additional_receiver.is_some() != additional_subject.is_some()
            || additional_receiver.is_some() != additional_body_path.is_some()
        {
            return Err(
                "additional_receiver, additional_subject and additional_body_path must be set together"
                    .to_string(),
            );
        }

        Ok(Self {
            from,
            from_name,
            subject,
            body_path,
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            additional_receiver,
            additional_subject,
            additional_body_path,
        })
    }
}

/// Replaces `{{key}}` placeholders in `template` with the values from `values`.
pub fn replace_placeholders(template: &str, values: &HashMap<String, String>) -> String {
    let mut result = template.to_owned();

    for (key, value) in values {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }

    result
}

fn build_mailer(mail_config: &MailConfig) -> Result<SmtpTransport, Box<dyn Error>> {
    let mut mailer = SmtpTransport::relay(&mail_config.smtp_host)?;
    if let Some(port) = mail_config.smtp_port {
        mailer = mailer.port(port);
    }
    if let (Some(username), Some(password)) = (&mail_config.smtp_username, &mail_config.smtp_password) {
        mailer = mailer.credentials(Credentials::new(username.clone(), password.clone()));
    }
    Ok(mailer.build())
}

fn build_email(
    mail_config: &MailConfig,
    to_name: Option<&str>,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<Message, Box<dyn Error>> {
    Ok(Message::builder()
        .from(Mailbox::new(mail_config.from_name.clone(), mail_config.from.parse()?))
        .to(Mailbox::new(to_name.map(str::to_owned), to.parse()?))
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_owned())?)
}

fn send_one(
    mailer: &SmtpTransport,
    mail_config: &MailConfig,
    to_name: Option<&str>,
    to: &str,
    subject: &str,
    body_path: &str,
    data: &HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let body = fs::read_to_string(body_path)
        .map_err(|e| format!("failed to read body file {body_path}: {e}"))?;
    let email = build_email(
        mail_config,
        to_name,
        to,
        &replace_placeholders(subject, data),
        &replace_placeholders(&body, data),
    )?;
    mailer.send(&email).map(|_| ()).map_err(Into::into)
}

/// Sends a mail to `to` (display name `to_name`) using the mail config matching the
/// lowercased name of the sqlite database `db` the submission was written to, falling
/// back to the `default` config.
///
/// If the matching mail config defines `additional_*` fields, a second mail is
/// always sent to that fixed recipient as well. The subject and body file of each
/// mail have their `{{placeholder}}`s substituted with the values from `data`.
///
/// # Errors
///
/// Fails if no matching mail config exists, a body file cannot be read, or a mail
/// cannot be sent over SMTP. Both mails are attempted even if one of them fails.
pub fn send_mail(
    config: &Config,
    to_name: &str,
    to: &str,
    db: &str,
    data: &HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let mail_config = config
        .mail_configs
        .get(&db.to_lowercase())
        .or_else(|| config.mail_configs.get("default"))
        .ok_or_else(|| format!("no mail config found for db {db}"))?;

    let mailer = build_mailer(mail_config)?;

    let primary = send_one(
        &mailer,
        mail_config,
        (!to_name.is_empty()).then_some(to_name),
        to,
        &mail_config.subject,
        &mail_config.body_path,
        data,
    );

    let additional = match (
        &mail_config.additional_receiver,
        &mail_config.additional_subject,
        &mail_config.additional_body_path,
    ) {
        (Some(receiver), Some(subject), Some(body_path)) => {
            send_one(&mailer, mail_config, None, receiver, subject, body_path, data)
        }
        _ => Ok(()),
    };

    primary.and(additional)
}

