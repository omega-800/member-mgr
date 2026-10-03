use std::{collections::HashMap, error::Error};

use crate::Config;

pub struct MailConfig {
    from: String,
    from_name: String,
    subject: String,
    body: String,
    smtp_username: String,
    smtp_password: String,
}

fn env_prefixes_to_map(prefix: &str) -> HashMap<String, String> {
    std::env::vars()
        .filter_map(|(key, value)| {
            let key = key.strip_prefix(prefix)?;

            Some((key.to_lowercase(), value))
        })
        .collect()
}

impl MailConfig {
    pub fn new(name: String) -> Self {
        Self 
    }
}

pub fn replace_placeholders(template: &str, values: &HashMap<String, String>) -> String {
    let mut result = template.to_owned();

    for (key, value) in values {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    result
}

pub fn send_mail(
    config: &Config,
    to_name: String,
    to: String,
) -> Result<lettre::transport::smtp::response::Response, Box<dyn Error>> {
    use lettre::{
        Message, SmtpTransport, Transport,
        message::{Mailbox, header::ContentType},
        transport::smtp::authentication::Credentials,
    };

    let email = Message::builder()
        .from(Mailbox::new(
            Some(config.from_name.clone()),
            config.from.parse()?,
        ))
        .to(Mailbox::new(Some(to_name.to_owned()), to.parse()?))
        .subject(config.subject.clone())
        .header(ContentType::TEXT_PLAIN)
        .body(config.body.clone())?;

    let creds = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    Ok(mailer.send(&email)?)
}
