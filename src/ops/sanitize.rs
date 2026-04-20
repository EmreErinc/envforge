use std::error::Error;
use std::path::Path;

/// Sanitize a file's content by replacing known secret values with ${KEY} placeholders.
///
/// Returns the sanitized content and the number of replacements made.
pub fn sanitize_content(
    content: &str,
    secrets: &[(String, String)],
) -> (String, usize) {
    let mut result = content.to_string();
    let mut count = 0;

    // Sort by value length descending (replace longest first to avoid partial matches)
    let mut sorted: Vec<_> = secrets
        .iter()
        .filter(|(_, v)| v.len() >= 4) // skip very short values
        .collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    for (key, value) in sorted {
        if result.contains(value.as_str()) {
            result = result.replace(value.as_str(), &format!("${{{}}}", key));
            count += 1;
        }
    }

    (result, count)
}

/// Sanitize a file on disk, replacing secret values with placeholders.
///
/// Reads the input file, replaces secrets, and writes to output (or returns the content).
pub fn sanitize_file(
    input_path: &Path,
    output_path: Option<&Path>,
    secrets: &[(String, String)],
) -> Result<usize, Box<dyn Error>> {
    let content = std::fs::read_to_string(input_path)?;
    let (sanitized, count) = sanitize_content(&content, secrets);

    if let Some(out) = output_path {
        std::fs::write(out, &sanitized)?;
    } else {
        print!("{}", sanitized);
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_replaces_values() {
        let content = "DATABASE_URL=postgres://user:supersecretpass@localhost/db\nAPI_KEY=sk-abcdef123456";
        let secrets = vec![
            ("DB_PASSWORD".to_string(), "supersecretpass".to_string()),
            ("API_KEY".to_string(), "sk-abcdef123456".to_string()),
        ];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 2);
        assert!(result.contains("${DB_PASSWORD}"));
        assert!(result.contains("${API_KEY}"));
        assert!(!result.contains("supersecretpass"));
        assert!(!result.contains("sk-abcdef123456"));
    }

    #[test]
    fn test_sanitize_skips_short_values() {
        let content = "PORT=80 and TOKEN=abc";
        let secrets = vec![
            ("PORT".to_string(), "80".to_string()),    // len 2, skip
            ("TOKEN".to_string(), "abc".to_string()),   // len 3, skip
        ];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 0);
        assert_eq!(result, content); // unchanged
    }

    #[test]
    fn test_sanitize_longest_first() {
        // If API_KEY_FULL="sk-abcdef123456" and API_KEY="sk-abcdef",
        // replacing longest first avoids partial match issues.
        let content = "token: sk-abcdef123456";
        let secrets = vec![
            ("API_KEY".to_string(), "sk-abcdef".to_string()),
            ("API_KEY_FULL".to_string(), "sk-abcdef123456".to_string()),
        ];

        let (result, count) = sanitize_content(content, &secrets);

        // Should replace the longest match first
        assert_eq!(count, 1);
        assert!(result.contains("${API_KEY_FULL}"));
        assert!(!result.contains("${API_KEY}")); // partial should NOT have matched
    }

    #[test]
    fn test_sanitize_no_secrets() {
        let content = "just some text without secrets";
        let secrets: Vec<(String, String)> = vec![];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 0);
        assert_eq!(result, content);
    }

    #[test]
    fn test_sanitize_multiple_occurrences() {
        let content = "first: mypassword123, second: mypassword123";
        let secrets = vec![("PASSWORD".to_string(), "mypassword123".to_string())];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 1); // one key replaced (even though multiple occurrences)
        assert_eq!(result, "first: ${PASSWORD}, second: ${PASSWORD}");
    }

    #[test]
    fn test_sanitize_file_to_output() {
        let tmp = tempfile::TempDir::new().unwrap();
        let input = tmp.path().join("input.txt");
        let output = tmp.path().join("output.txt");

        std::fs::write(&input, "secret=mysupersecretvalue").unwrap();

        let secrets = vec![("SECRET".to_string(), "mysupersecretvalue".to_string())];
        let count = sanitize_file(&input, Some(&output), &secrets).unwrap();

        assert_eq!(count, 1);
        let result = std::fs::read_to_string(&output).unwrap();
        assert!(result.contains("${SECRET}"));
        assert!(!result.contains("mysupersecretvalue"));
    }
}
