use crate::model::LineNode;

/// A single undo entry — captures the state of a file before a mutation.
#[derive(Debug)]
pub struct UndoEntry {
    /// Index into the App's shell_files vector
    pub file_index: usize,
    /// Snapshot of the file's lines before the operation
    pub lines_snapshot: Vec<LineNode>,
    /// Human-readable description of what was done
    pub description: String,
}

/// In-session undo stack.
#[derive(Debug)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a snapshot before performing a mutation.
    pub fn push(&mut self, file_index: usize, lines: &[LineNode], description: &str) {
        self.entries.push(UndoEntry {
            file_index,
            lines_snapshot: clone_lines(lines),
            description: description.to_string(),
        });
    }

    /// Pop the last entry and return it for restoration.
    pub fn pop(&mut self) -> Option<UndoEntry> {
        self.entries.pop()
    }

    /// Number of undoable operations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the undo stack (e.g., after save).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Peek at the last entry's description without removing it.
    pub fn last_description(&self) -> Option<&str> {
        self.entries.last().map(|e| e.description.as_str())
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Clone a Vec<LineNode> for snapshot purposes.
///
/// LineNode derives Clone, so this is straightforward.
fn clone_lines(lines: &[LineNode]) -> Vec<LineNode> {
    lines.to_vec()
}
