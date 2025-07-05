use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset, Local, TimeZone};

pub enum SignatureKind {
    Author,
    Committer,
}

#[derive(Clone)]
pub struct Signature {
    name: String,
    email: String,
    timestamp: DateTime<FixedOffset>,
}

impl Signature {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            timestamp: Local::now().fixed_offset(),
        }
    }

    pub fn serialize_as(&self, kind: SignatureKind) -> String {
        let kind = match kind {
            SignatureKind::Author => "author",
            SignatureKind::Committer => "committer",
        };
        format!(
            "{} {} <{}> {} {}",
            kind,
            self.name,
            self.email,
            self.timestamp.timestamp(),
            format_offset(self.timestamp.offset().local_minus_utc())
        )
    }

    pub fn deserialize(serialized: &str) -> Result<Self> {
        let mut parts = serialized.split_whitespace().peekable();

        // Skip the "author" or "committer" part
        parts.next().context("Missing signature type")?;

        // Parse name (may contain spaces)
        let mut name_parts = Vec::new();
        while let Some(part) = parts.peek() {
            if part.starts_with('<') {
                // We've reached the email part
                break;
            }
            name_parts.push(part.to_string());
            parts.next().unwrap();
        }
        let name = name_parts.join(" ");
        if name.is_empty() {
            bail!("Missing author name");
        }

        // Parse email (remove < and >)
        let email_part = parts.next().context("Missing email")?;
        if !email_part.starts_with('<') || !email_part.ends_with('>') {
            bail!("Invalid email format");
        }
        let email = email_part[1..email_part.len() - 1].to_string();

        // Parse timestamp
        let timestamp_str = parts.next().context("Missing timestamp")?;
        let timestamp = timestamp_str.parse::<i64>().context("Invalid timestamp")?;

        // Parse offset
        let offset_str = parts.next().context("Missing offset")?;
        let offset_seconds = parse_offset(offset_str)?;
        let offset = FixedOffset::east_opt(offset_seconds).context("Invalid offset")?;

        let timestamp = offset
            .timestamp_opt(timestamp, 0)
            .single()
            .context("Invalid timestamp")?;

        Ok(Self {
            name,
            email,
            timestamp,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn email(&self) -> &str {
        &self.email
    }
}

fn format_offset(offset_seconds: i32) -> String {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let offset_minutes = offset_seconds.abs() / 60;
    let hours = offset_minutes / 60;
    let minutes = offset_minutes % 60;
    format!("{sign}{hours:02}{minutes:02}")
}

fn parse_offset(offset: &str) -> Result<i32> {
    if offset.len() != 5 {
        anyhow::bail!("Invalid offset format");
    }
    let sign = match &offset[0..1] {
        "+" => 1,
        "-" => -1,
        _ => anyhow::bail!("Invalid offset sign"),
    };
    let hours: i32 = offset[1..3].parse()?;
    let minutes: i32 = offset[3..5].parse()?;
    Ok(sign * (hours * 3600 + minutes * 60))
}
