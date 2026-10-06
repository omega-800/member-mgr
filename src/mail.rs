use std::{collections::HashMap, fs, error::Error};

use crate::Config;

pub struct MailConfig {
    from: String,
    from_name: String,
    subject: String,
    body_path: String,
    smtp_username: String,
    smtp_password: String,
}

impl MailConfig {
    pub fn new(name: String) -> Self {
        let mut configs = env_prefixes_to_map("MAIL_CFG_");
        
        let config_str = configs.remove(&name.to_lowercase()).unwrap_or_default();
        let values: Vec<&str> = config_str.as_str().split(';').filter(|s| !s.is_empty()).collect();

        let from = if let Some(value) = values.iter().find(|v| v.starts_with("from=")).map(|v| &v[5..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else {
 "".to_string()
        };

        let from_name = if let Some(value) = values.iter().find(|v| v.starts_with("from_name=")).map(|v| &v[9..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else if !from.is_empty() {
            "Web Service".to_string()
        } else {
            "".to_string()
        };

        let subject = if let Some(value) = values.iter().find(|v| v.starts_with("subject=")).map(|v| &v[8..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else {
            "".to_string()
        };

        let body_path = if let Some(value) = values.iter().find(|v| v.starts_with("body_path=")).map(|v| &v[10..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else {
            "".to_string()
        };

        let smtp_username = if let Some(value) = values.iter().find(|v| v.starts_with("smtp_username=")).map(|v| &v[13..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else {
            "".to_string()
        };

        let smtp_password = if let Some(value) = values.iter().find(|v| v.starts_with("smtp_password=")).map(|v| &v[15..]).filter(|v| !v.is_empty()) {
            value.to_string()
        } else {
            "".to_string()
        };

        Self {
            from,
            from_name,
            subject,
            body_path,
            smtp_username,
            smtp_password,
        }
    }
}

fn env_prefixes_to_map(prefix: &str) -> HashMap<String, String> {
    std::env::vars()
        .filter_map(|(key, value)| {
            let key = key.strip_prefix(prefix)?;
            Some((key.to_lowercase(), value))
        })
        .collect()
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
    subject_data: &HashMap<String, String>,
    body_data: &HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    #[cfg(not(feature = "mail"))]
    {
        return Err("Mail feature not enabled".into());
    }

    use lettre::{
        Message, SmtpTransport, Transport,
        message::header::ContentType,
        transport::smtp::authentication::Credentials,
    };

    let mail_config: &MailConfig = config.mail_configs.get(&to)
        .or_else(|| config.mail_configs.get("default"))
        .unwrap_or_else(|| panic!("No mail config found for: {}", to));

    let subject = if !mail_config.subject.is_empty() {
        replace_placeholders(&mail_config.subject, subject_data)
    } else {
        String::new()
    };

    let body = if !mail_config.body_path.is_empty() {
        match fs::read_to_string(&mail_config.body_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("[WARN!] Failed to read body file {}: {}", mail_config.body_path, e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    let formatted_body = replace_placeholders(&body, body_data);

    let email = Message::builder()
        .from(Mailbox::new(
            Some(mail_config.from_name.clone()),
            mail_config.from.parse()?,
        ))
        .to(Mailbox::new(Some(to_name.to_owned()), to.parse()?))
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(formatted_body)?;

    let creds = Credentials::new(mail_config.smtp_username.clone(), mail_config.smtp_password.clone());

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    Ok(mailer.send(&email)?)
}