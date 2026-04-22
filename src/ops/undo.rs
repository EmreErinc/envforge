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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_lines(n: usize) -> Vec<LineNode> {
        (0..n)
            .map(|i| LineNode::Blank {
                line_number: i,
                original_text: String::new(),
            })
            .collect()
    }

    #[test]
    fn test_new_stack_is_empty() {
        let stack = UndoStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_push_pop_lifo() {
        let mut stack = UndoStack::new();
        stack.push(0, &make_test_lines(1), "first");
        stack.push(1, &make_test_lines(2), "second");

        let entry = stack.pop().unwrap();
        assert_eq!(entry.description, "second");
        assert_eq!(entry.file_index, 1);

        let entry = stack.pop().unwrap();
        assert_eq!(entry.description, "first");
        assert_eq!(entry.file_index, 0);
    }

    #[test]
    fn test_pop_empty_returns_none() {
        let mut stack = UndoStack::new();
        assert!(stack.pop().is_none());
    }

    #[test]
    fn test_len_tracks_entries() {
        let mut stack = UndoStack::new();
        stack.push(0, &make_test_lines(1), "a");
        stack.push(0, &make_test_lines(1), "b");
        stack.push(0, &make_test_lines(1), "c");
        assert_eq!(stack.len(), 3);
    }

    #[test]
    fn test_clear_empties_stack() {
        let mut stack = UndoStack::new();
        stack.push(0, &make_test_lines(1), "a");
        stack.push(0, &make_test_lines(1), "b");
        stack.clear();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_last_description() {
        let mut stack = UndoStack::new();
        stack.push(0, &make_test_lines(1), "edit FOO");
        assert_eq!(stack.last_description(), Some("edit FOO"));
    }

    #[test]
    fn test_last_description_empty_stack() {
        let stack = UndoStack::new();
        assert_eq!(stack.last_description(), None);
    }

    #[test]
    fn test_push_preserves_snapshot() {
        let mut stack = UndoStack::new();
        let lines = make_test_lines(3);
        stack.push(2, &lines, "snapshot test");

        let entry = stack.pop().unwrap();
        assert_eq!(entry.file_index, 2);
        assert_eq!(entry.lines_snapshot.len(), 3);
    }
}
