/// Maximum bytes accepted in a single TUI text-input field.
/// Bracketed-paste in modern terminals can deliver megabytes in one
/// event; without a cap, the backing `String` grows unbounded and a
/// crafted paste can OOM the process. 128 KiB comfortably exceeds any
/// legitimate ENV value, schema doc, or commit message edited in-app.
pub const MAX_INPUT_LEN: usize = 128 * 1024;

/// Simple text input state for edit/add dialogs.
#[derive(Debug, Clone)]
pub struct TextInput {
    pub content: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(initial: &str) -> Self {
        // Truncate any oversized initial text up front so the invariant
        // `content.len() <= MAX_INPUT_LEN` holds for the lifetime of the
        // struct.
        let mut content = initial.to_string();
        if content.len() > MAX_INPUT_LEN {
            // Truncate at a char boundary to keep the String valid.
            let mut cut = MAX_INPUT_LEN;
            while cut > 0 && !content.is_char_boundary(cut) {
                cut -= 1;
            }
            content.truncate(cut);
        }
        Self {
            cursor: content.len(),
            content,
        }
    }

    pub fn empty() -> Self {
        Self::new("")
    }

    pub fn insert(&mut self, ch: char) {
        // Refuse insertion that would exceed the size cap. Silent no-op
        // matches the existing "best-effort input" feel of the TUI.
        if self.content.len() + ch.len_utf8() > MAX_INPUT_LEN {
            return;
        }
        self.content.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.content.len() {
            self.content.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            self.cursor += self.content[self.cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.content.len();
    }

    pub fn value(&self) -> &str {
        &self.content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction ──────────────────────────────────────

    #[test]
    fn test_new_sets_cursor_to_end() {
        let input = TextInput::new("hello");
        assert_eq!(input.value(), "hello");
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn test_empty_returns_empty_input() {
        let input = TextInput::empty();
        assert_eq!(input.value(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_new_truncates_oversized_initial_text() {
        let oversized = "x".repeat(MAX_INPUT_LEN + 100);
        let input = TextInput::new(&oversized);
        assert!(input.content.len() <= MAX_INPUT_LEN);
    }

    #[test]
    fn test_new_unicode_truncation_at_char_boundary() {
        // Build a string at MAX_INPUT_LEN + some extra unicode chars
        let prefix = "x".repeat(MAX_INPUT_LEN);
        let content = format!("{}🦀", prefix);
        let input = TextInput::new(&content);
        // Should truncate at a valid char boundary
        assert!(input.content.len() <= MAX_INPUT_LEN);
    }

    // ── Insert ────────────────────────────────────────────

    #[test]
    fn test_insert_basic() {
        let mut input = TextInput::empty();
        input.insert('a');
        assert_eq!(input.value(), "a");
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn test_insert_middle() {
        let mut input = TextInput::new("ac");
        input.cursor = 1;
        input.insert('b');
        assert_eq!(input.value(), "abc");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_insert_unicode_multi_byte() {
        let mut input = TextInput::new("start end");
        input.cursor = 6;
        input.insert('🦀');
        assert_eq!(input.value(), "start 🦀end");
        // cursor should advance by the byte length of the emoji (4 bytes)
        assert_eq!(input.cursor, 10);
    }

    #[test]
    fn test_insert_at_max_len_is_noop() {
        let mut input = TextInput::new(&"x".repeat(MAX_INPUT_LEN));
        let old_len = input.content.len();
        input.insert('y');
        assert_eq!(input.content.len(), old_len);
    }

    #[test]
    fn test_insert_near_max_len_rejected_if_would_exceed() {
        let mut input = TextInput::new(&"x".repeat(MAX_INPUT_LEN - 1));
        input.insert('a'); // 1 byte, fits
        assert_eq!(input.content.len(), MAX_INPUT_LEN);
        input.insert('b'); // 1 byte, would exceed
        assert_eq!(input.content.len(), MAX_INPUT_LEN); // unchanged
    }

    // ── Backspace ─────────────────────────────────────────

    #[test]
    fn test_backspace_basic() {
        let mut input = TextInput::new("abc");
        input.backspace();
        assert_eq!(input.value(), "ab");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_backspace_at_start_is_noop() {
        let mut input = TextInput::new("abc");
        input.cursor = 0;
        input.backspace();
        assert_eq!(input.value(), "abc");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_backspace_empty_is_noop() {
        let mut input = TextInput::empty();
        input.backspace();
        assert_eq!(input.value(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_backspace_unicode_char() {
        // "a🦀b" = 1 + 4 + 1 = 6 bytes. Cursor starts at 6.
        let mut input = TextInput::new("a🦀b");
        input.backspace(); // removes 'b' (at byte index 5)
        assert_eq!(input.value(), "a🦀");
        assert_eq!(input.cursor, 5);
        input.backspace(); // removes the emoji '🦀' (at byte index 1)
        assert_eq!(input.value(), "a");
        assert_eq!(input.cursor, 1);
    }

    // ── Delete ────────────────────────────────────────────

    #[test]
    fn test_delete_basic() {
        let mut input = TextInput::new("abc");
        input.cursor = 1;
        input.delete();
        assert_eq!(input.value(), "ac");
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn test_delete_at_end_is_noop() {
        let mut input = TextInput::new("abc");
        input.delete();
        assert_eq!(input.value(), "abc");
    }

    #[test]
    fn test_delete_unicode_char() {
        let mut input = TextInput::new("a🦀b");
        input.cursor = 1;
        input.delete();
        assert_eq!(input.value(), "ab");
        assert_eq!(input.cursor, 1);
    }

    // ── Cursor Movement ───────────────────────────────────

    #[test]
    fn test_move_left_basic() {
        let mut input = TextInput::new("abc");
        input.move_left();
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_move_left_at_start_is_noop() {
        let mut input = TextInput::new("abc");
        input.cursor = 0;
        input.move_left();
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_move_left_across_unicode() {
        // "a🦀b" = 6 bytes: a[0], 🦀[1-4], b[5]
        let mut input = TextInput::new("a🦀b");
        input.move_left(); // cursor from 6 to before 'b' (byte index 5)
        assert_eq!(input.cursor, 5);
        input.move_left(); // cursor from 5 to before '🦀' (byte index 1)
        assert_eq!(input.cursor, 1);
        input.move_left(); // cursor from 1 to before 'a' (byte index 0)
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_move_right_basic() {
        let mut input = TextInput::new("abc");
        input.cursor = 1;
        input.move_right();
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_move_right_at_end_is_noop() {
        let mut input = TextInput::new("abc");
        input.move_right();
        assert_eq!(input.cursor, 3);
    }

    #[test]
    fn test_move_right_across_unicode() {
        let mut input = TextInput::new("a🦀b");
        input.cursor = 0;
        input.move_right();
        assert_eq!(input.cursor, 1); // 'a' is 1 byte
        input.move_right();
        assert_eq!(input.cursor, 5); // '🦀' is 4 bytes
    }

    #[test]
    fn test_move_home() {
        let mut input = TextInput::new("abc");
        input.move_home();
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_move_end() {
        let mut input = TextInput::new("abc");
        input.cursor = 0;
        input.move_end();
        assert_eq!(input.cursor, 3);
    }

    // ── Integration ───────────────────────────────────────

    #[test]
    fn test_edit_sequence() {
        let mut input = TextInput::empty();
        input.insert('h');
        input.insert('e');
        input.insert('l');
        input.insert('l');
        input.insert('o');
        assert_eq!(input.value(), "hello");

        input.move_left();
        input.move_left();
        input.backspace();
        assert_eq!(input.value(), "helo");

        input.insert('l');
        assert_eq!(input.value(), "hello");
    }

    #[test]
    fn test_insert_delete_unicode_sequence() {
        let mut input = TextInput::new("rust ");
        input.insert('🦀');
        input.insert(' ');
        input.insert('l');
        input.insert('a');
        input.insert('n');
        input.insert('g');
        assert_eq!(input.value(), "rust 🦀 lang");
    }

    #[test]
    fn test_max_len_boundary_unicode() {
        // Fill to almost MAX with ASCII, then try to insert multi-byte char
        let mut input = TextInput::new(&"x".repeat(MAX_INPUT_LEN - 2));
        input.cursor = input.content.len();
        input.insert('🦀'); // 4 bytes, would exceed MAX
                            // Should be rejected — content unchanged
        assert!(input.content.len() <= MAX_INPUT_LEN);
        assert!(!input.value().contains('🦀'));
    }
}
