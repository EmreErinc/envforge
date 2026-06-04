pub fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&p) = chars.peek() {
                    if p.is_ascii_digit()
                        || p == ';'
                        || p == ':'
                        || p == '<'
                        || p == '='
                        || p == '>'
                        || p == '?'
                        || p == ' '
                    {
                        chars.next();
                    } else {
                        break;
                    }
                }
                while let Some(&p) = chars.peek() {
                    #[allow(clippy::manual_range_contains)]
                    if (p >= ' ' && p <= '/') || p == '@' {
                        chars.next();
                        if p == 'm'
                            || p == 'M'
                            || p == 'H'
                            || p == 'A'
                            || p == 'B'
                            || p == 'C'
                            || p == 'D'
                            || p == 'J'
                            || p == 'K'
                            || p == 'h'
                            || p == 'l'
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                chars.next();
            } else if chars.peek() == Some(&']') {
                chars.next();
                while let Some(&p) = chars.peek() {
                    chars.next();
                    if p == '\x07' || p == '\x1b' {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }

    out
}

pub fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

pub fn sanitize_for_display(s: &str) -> String {
    strip_ansi_escapes(&strip_control_chars(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_csi() {
        assert_eq!(strip_ansi_escapes("\x1b[31mhello\x1b[0m"), "hello");
    }

    #[test]
    fn test_strip_ansi_complex() {
        assert_eq!(strip_ansi_escapes("\x1b[1;31;42mcolored\x1b[0m"), "colored");
    }

    #[test]
    fn test_preserve_normal_text() {
        assert_eq!(strip_ansi_escapes("hello world"), "hello world");
    }

    #[test]
    fn test_strip_osc() {
        assert_eq!(strip_ansi_escapes("\x1b]0;title\x07hello"), "hello");
    }

    #[test]
    fn test_strip_control_chars_preserves_newlines() {
        assert_eq!(strip_control_chars("hello\nworld\t!"), "hello\nworld\t!");
    }

    #[test]
    fn test_strip_control_chars_removes_bell() {
        assert_eq!(strip_control_chars("beep\x07go"), "beepgo");
    }

    #[test]
    fn test_sanitize_pipeline() {
        assert_eq!(sanitize_for_display("safe text"), "safe text");
    }
}
