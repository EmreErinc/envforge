use regex::Regex;
use std::sync::OnceLock;

static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
static URL_REGEX: OnceLock<Regex> = OnceLock::new();

/// Validate an email address with a reasonably good regex.
pub fn is_valid_email(email: &str) -> bool {
    let re = EMAIL_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap());
    re.is_match(email)
}

/// Validate a URL. Supports http, https, and common DB protocols.
pub fn is_valid_url(url: &str) -> bool {
    // Simple check first
    if url.is_empty() {
        return false;
    }

    let protocols = [
        "http://",
        "https://",
        "postgres://",
        "mysql://",
        "redis://",
        "mongodb://",
        "amqp://",
        "ssh://",
        "git://",
        "s3://",
    ];

    if !protocols.iter().any(|p| url.starts_with(p)) {
        return false;
    }

    // Basic regex for the rest of the URL
    let re = URL_REGEX.get_or_init(|| Regex::new(r"^[a-z0-9]+://[^\s/$.?#].[^\s]*$").unwrap());
    re.is_match(url)
}

/// Validate if a string is a valid port number (1-65535).
pub fn is_valid_port(port: &str) -> bool {
    port.parse::<u16>().is_ok_and(|p| p > 0)
}

pub fn is_valid_bool(val: &str) -> bool {
    matches!(
        val.to_lowercase().as_str(),
        "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("a@b.cd"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("test@"));
        assert!(!is_valid_email("test@example"));
    }

    #[test]
    fn test_is_valid_url() {
        assert!(is_valid_url("https://google.com"));
        assert!(is_valid_url("s3://bucket/key"));
        assert!(!is_valid_url("ftp://site.com"));
        assert!(!is_valid_url("https://"));
    }
}
