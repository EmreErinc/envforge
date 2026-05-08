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
