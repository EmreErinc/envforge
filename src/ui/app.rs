use std::collections::HashSet;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::{load_or_create_default, safe_write, AppConfig};
use crate::model::{ExportStyle, QuoteStyle, ShellFile};
use crate::ops::*;
use crate::parser::{parse_shell_file, serialize_shell_file};

use super::input::TextInput;
use super::render;

/// The current mode of the TUI.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Normal,
    Editing,
    Adding(AddField),
    Searching,
    Confirming(ConfirmAction),
    DiffPreview,
    Help,
    Importing,
    Exporting,
    ProfileSelector(usize), // index in profile list
    FirstRun,               // first-run security setup wizard
}

/// Which field is active in the Add dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum AddField {
    Key,
    Value,
}

/// Target for add/move operations.
#[derive(Debug, Clone, PartialEq)]
pub enum AddTarget {
    Profile,
    Shared,
}

/// What action requires confirmation.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Delete(String),
    Move(String),
    Save,
    Quit,
}

/// A status bar notification.
#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub level: NotificationLevel,
    pub created: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Success,
    Warning,
    Error,
}

/// A row in the TUI table — either a group header or an entry.
#[derive(Debug, Clone)]
pub enum TableRow {
    GroupHeader {
        name: String,
        count: usize,
        collapsed: bool,
    },
    Entry(EnvEntry),
}

/// Main application state.
pub struct App {
    pub entries: Vec<EnvEntry>,
    pub shell_files: Vec<ShellFile>,
    pub config: AppConfig,
    pub selected: usize,
    pub mode: ViewMode,
    pub search_query: String,
    pub input: TextInput,
    pub add_key_input: TextInput,
    pub add_value_input: TextInput,
    pub notification: Option<Notification>,
    /// Keys (not row indices) whose value is currently revealed. Keyed by the
    /// env var name so scrolling/sorting/regrouping can't reveal the wrong
    /// secret when a row index is later reused (L9).
    pub revealed: HashSet<String>,
    pub should_quit: bool,
    pub diff_content: String,
    pub has_unsaved_changes: bool,
    pub duplicate_keys: std::collections::HashSet<String>,
    pub undo_stack: UndoStack,
    pub collapsed_groups: HashSet<String>,
    pub grouping_enabled: bool,
    pub add_target: AddTarget,
    pub help_page: usize,
    /// Lifecycle info message displayed when 'L' is pressed
    pub lifecycle_info: String,
    /// Whether the AI tool fence is currently active for this session
    pub fence_enabled: bool,
    /// Whether the first-run security setup has been completed
    pub first_run_completed: bool,
    /// Resolved fence targets for the current config — shown read-only in the footer (FR16).
    /// Populated once on construction; refreshed when the fence is toggled.
    pub fence_resolved_targets: Vec<crate::ops::fence::ResolvedTarget>,
}

impl App {
    /// Create a new App by loading config and parsing shell files.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = load_or_create_default()?;

        // Detect first run — config file was just created by load_or_create_default
        let is_first_run = crate::config::config_file_path()
            .map(|p| {
                // Check if the config was freshly created (file is new or has first_run not set)
                p.exists()
                    && std::fs::metadata(&p)
                        .map(|m| {
                            m.created()
                                .or_else(|_| m.modified())
                                .map(|t| t.elapsed().map(|d| d.as_secs() < 5).unwrap_or(false))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);

        let primary_path = shellexpand_path(&config.files.primary);
        let mut shell_files = Vec::new();

        // Index 0: primary shell config (.zshrc)
        if primary_path.exists() {
            shell_files.push(parse_shell_file(&primary_path)?);
        }

        // Index 1: shared ENV file (~/.env_managed.shared)
        let shared_path = shellexpand_path(&config.profiles.shared_file);
        if shared_path.exists() {
            shell_files.push(parse_shell_file(&shared_path)?);
        } else {
            // Create empty shared file so it's always available
            std::fs::write(&shared_path, "# EnvForge shared environment variables\n")?;
            shell_files.push(parse_shell_file(&shared_path)?);
        }

        // Index 2: active profile file (~/.env_managed.{profile})
        if let Some(profile_file) = config.profiles.active_file() {
            let profile_path = shellexpand_path(&profile_file);
            if profile_path.exists() {
                shell_files.push(parse_shell_file(&profile_path)?);
            } else {
                // Create empty profile file
                std::fs::write(
                    &profile_path,
                    format!("# EnvForge profile: {}\n", config.profiles.active),
                )?;
                shell_files.push(parse_shell_file(&profile_path)?);
            }
        }

        let entries = collect_all_entries(&shell_files);
        let duplicate_keys = duplicate_key_set(&shell_files);

        // Pre-compute collapsed groups: all collapsed except "Other"
        let group_config = GroupConfig {
            groups: config
                .groups
                .iter()
                .map(|(n, p)| (n.clone(), p.clone()))
                .collect(),
        };
        let groups = group_entries(&entries, &group_config);
        let mut collapsed_groups = HashSet::new();
        for group in &groups {
            if group.name != "Other" {
                collapsed_groups.insert(group.name.clone());
            }
        }

        // Resolve fence targets once at startup for the read-only footer display (FR16).
        let fence_resolved_targets = {
            let fence_cfg = crate::config::load_or_create_default()
                .map(|c| c.fence)
                .unwrap_or_default();
            crate::ops::fence::resolve_fence_targets(&fence_cfg)
        };

        Ok(Self {
            entries,
            shell_files,
            config,
            selected: 0,
            mode: if is_first_run {
                ViewMode::FirstRun
            } else {
                ViewMode::Normal
            },
            search_query: String::new(),
            input: TextInput::empty(),
            add_key_input: TextInput::empty(),
            add_value_input: TextInput::empty(),
            notification: None,
            revealed: HashSet::new(),
            should_quit: false,
            diff_content: String::new(),
            has_unsaved_changes: false,
            duplicate_keys,
            undo_stack: UndoStack::new(),
            collapsed_groups,
            grouping_enabled: true,
            add_target: AddTarget::Profile,
            help_page: 0,
            lifecycle_info: String::new(),
            fence_enabled: false,
            first_run_completed: false,
            fence_resolved_targets,
        })
    }

    /// Get the filtered entries based on current search query (uses fuzzy matching).
    pub fn visible_entries(&self) -> Vec<EnvEntry> {
        if self.search_query.is_empty() {
            self.entries.clone()
        } else {
            fuzzy_search(&self.entries, &self.search_query)
                .into_iter()
                .map(|m| m.entry)
                .collect()
        }
    }

    /// Get fuzzy match results with indices for highlighting.
    pub fn fuzzy_results(&self) -> Vec<FuzzyMatch> {
        if self.search_query.is_empty() {
            vec![]
        } else {
            fuzzy_search(&self.entries, &self.search_query)
        }
    }

    /// Build grouped table rows for TUI display.
    ///
    /// When search is active or grouping disabled, returns flat list.
    /// Otherwise, returns group headers + entries with collapse support.
    pub fn visible_rows(&self) -> Vec<TableRow> {
        let entries = self.visible_entries();

        // Disable grouping during search
        if !self.grouping_enabled || !self.search_query.is_empty() {
            return entries.into_iter().map(TableRow::Entry).collect();
        }

        let group_config = self.build_group_config();
        let groups = group_entries(&entries, &group_config);

        let mut rows = Vec::new();
        for group in &groups {
            let collapsed = self.collapsed_groups.contains(&group.name);
            rows.push(TableRow::GroupHeader {
                name: group.name.clone(),
                count: group.entries.len(),
                collapsed,
            });
            if !collapsed {
                for entry in &group.entries {
                    rows.push(TableRow::Entry(entry.clone()));
                }
            }
        }
        rows
    }

    /// Build GroupConfig from AppConfig.
    fn build_group_config(&self) -> GroupConfig {
        let groups: Vec<(String, Vec<String>)> = self
            .config
            .groups
            .iter()
            .map(|(name, patterns)| (name.clone(), patterns.clone()))
            .collect();
        GroupConfig { groups }
    }

    /// Get the selected entry (skipping group headers).
    pub fn selected_entry(&self) -> Option<EnvEntry> {
        let rows = self.visible_rows();
        rows.get(self.selected).and_then(|row| match row {
            TableRow::Entry(e) => Some(e.clone()),
            TableRow::GroupHeader { .. } => None,
        })
    }

    /// Check if the selected row is a group header.
    pub fn selected_is_header(&self) -> bool {
        let rows = self.visible_rows();
        matches!(rows.get(self.selected), Some(TableRow::GroupHeader { .. }))
    }

    /// Check if a value should be masked.
    pub fn is_masked(&self, key: &str) -> bool {
        // Reveal is tracked by key identity (L9), not row index.
        if self.revealed.contains(key) {
            return false;
        }
        // Delegate to the canonical key-sensitivity decision (FR6 / L6 / L12).
        crate::ops::dotenv::is_sensitive_key(key)
    }

    /// Set a notification message.
    pub fn notify(&mut self, message: &str, level: NotificationLevel) {
        self.notification = Some(Notification {
            message: message.to_string(),
            level,
            created: Instant::now(),
        });
    }

    /// Refresh entries from shell files.
    /// Get the file index for the active profile (index 2).
    pub fn profile_file_index(&self) -> usize {
        2
    }

    /// Get the file index for the shared file (index 1).
    pub fn shared_file_index(&self) -> usize {
        1
    }

    /// Get the file index for the primary shell config (index 0).
    pub fn primary_file_index(&self) -> usize {
        0
    }

    /// Get the target file index based on add_target.
    pub fn target_file_index(&self) -> usize {
        match self.add_target {
            AddTarget::Profile => self.profile_file_index(),
            AddTarget::Shared => self.shared_file_index(),
        }
    }

    /// Snapshot a file's state before mutation for undo.
    fn snapshot(&mut self, file_index: usize, description: &str) {
        if let Some(sf) = self.shell_files.get(file_index) {
            self.undo_stack.push(file_index, &sf.lines, description);
        }
    }

    fn refresh_entries(&mut self) {
        self.entries = collect_all_entries(&self.shell_files);
        self.duplicate_keys = duplicate_key_set(&self.shell_files);
    }

    /// Handle a key event based on current mode.
    /// Handle mouse events.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        // Only handle mouse in Normal mode
        if self.mode != ViewMode::Normal {
            return;
        }

        let rows = self.visible_rows();
        let row_count = rows.len();

        match mouse.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                // Click: select row (offset by header area: 3 lines header + 1 table header)
                let table_start = 4_u16; // header border(3) + table header row(1)
                if mouse.row >= table_start {
                    let clicked_row = (mouse.row - table_start) as usize;
                    if clicked_row < row_count {
                        self.selected = clicked_row;

                        // If clicked on group header, toggle collapse
                        if let Some(TableRow::GroupHeader {
                            name, collapsed, ..
                        }) = rows.get(clicked_row)
                        {
                            if *collapsed {
                                self.collapsed_groups.remove(name);
                            } else {
                                self.collapsed_groups.insert(name.clone());
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp if self.selected > 0 => {
                self.selected -= 1;
            }
            MouseEventKind::ScrollDown if row_count > 0 && self.selected < row_count - 1 => {
                self.selected += 1;
            }
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Clear stale notifications (>3s)
        if let Some(notif) = &self.notification {
            if notif.created.elapsed() > Duration::from_secs(3) {
                self.notification = None;
            }
        }

        match &self.mode.clone() {
            ViewMode::Normal => self.handle_normal_key(key),
            ViewMode::Editing => self.handle_edit_key(key),
            ViewMode::Adding(field) => self.handle_add_key(key, field.clone()),
            ViewMode::Searching => self.handle_search_key(key),
            ViewMode::Confirming(action) => self.handle_confirm_key(key, action.clone()),
            ViewMode::DiffPreview => self.handle_diff_key(key),
            ViewMode::Help => self.handle_help_key(key),
            ViewMode::Importing => self.handle_import_key(key),
            ViewMode::Exporting => self.handle_export_key(key),
            ViewMode::ProfileSelector(idx) => self.handle_profile_selector_key(key, *idx),
            ViewMode::FirstRun => self.handle_first_run_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        let rows = self.visible_rows();
        let row_count = rows.len();

        match key.code {
            KeyCode::Char('q') => {
                if self.has_unsaved_changes {
                    self.mode = ViewMode::Confirming(ConfirmAction::Quit);
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('j') | KeyCode::Down
                if row_count > 0 && self.selected < row_count - 1 =>
            {
                self.selected += 1;
            }
            KeyCode::Char('k') | KeyCode::Up if self.selected > 0 => {
                self.selected -= 1;
            }
            // Enter/Right on group header = expand, on entry = no-op
            KeyCode::Enter | KeyCode::Right => {
                if let Some(TableRow::GroupHeader {
                    name, collapsed, ..
                }) = rows.get(self.selected)
                {
                    if *collapsed {
                        self.collapsed_groups.remove(name);
                    } else {
                        // Already expanded — no-op
                    }
                }
            }
            // Left on group header = collapse
            KeyCode::Left => {
                if let Some(TableRow::GroupHeader {
                    name, collapsed, ..
                }) = rows.get(self.selected)
                {
                    if !*collapsed {
                        self.collapsed_groups.insert(name.clone());
                    }
                }
            }
            KeyCode::Char('g') => {
                // Toggle grouping on/off
                self.grouping_enabled = !self.grouping_enabled;
                self.selected = 0;
                self.notify(
                    if self.grouping_enabled {
                        "Grouping enabled"
                    } else {
                        "Grouping disabled"
                    },
                    NotificationLevel::Success,
                );
            }
            KeyCode::Char(' ') => {
                // Toggle active/passive for selected entry
                if let Some(entry) = self.selected_entry() {
                    let source = entry.source_file.clone();
                    let key_name = entry.key.clone();
                    if let Some(fi) = self.shell_files.iter().position(|sf| sf.path == source) {
                        if entry.location == EntryLocation::Commented {
                            // Passive → Active: undo delete
                            self.snapshot(fi, &format!("Activate {}", key_name));
                            match undo_delete(&mut self.shell_files[fi], &key_name) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify(
                                        &format!("Activated: {}", key_name),
                                        NotificationLevel::Success,
                                    );
                                }
                                Err(e) => {
                                    self.undo_stack.pop();
                                    self.notify(&e.to_string(), NotificationLevel::Error);
                                }
                            }
                        } else {
                            // Active → Passive: soft delete
                            self.snapshot(fi, &format!("Deactivate {}", key_name));
                            match soft_delete(&mut self.shell_files[fi], &key_name) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify(
                                        &format!("Deactivated: {}", key_name),
                                        NotificationLevel::Success,
                                    );
                                }
                                Err(e) => {
                                    self.undo_stack.pop();
                                    self.notify(&e.to_string(), NotificationLevel::Error);
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('e') => {
                if let Some(entry) = self.selected_entry() {
                    if entry.location != EntryLocation::Commented {
                        self.input = TextInput::new(&entry.value);
                        self.mode = ViewMode::Editing;
                    }
                }
            }
            KeyCode::Char('a') => {
                self.add_key_input = TextInput::empty();
                self.add_value_input = TextInput::empty();
                self.mode = ViewMode::Adding(AddField::Key);
            }
            KeyCode::Char('d') => {
                if let Some(entry) = self.selected_entry() {
                    if entry.location == EntryLocation::InFile {
                        self.mode = ViewMode::Confirming(ConfirmAction::Delete(entry.key));
                    }
                }
            }
            KeyCode::Char('m') => {
                if let Some(entry) = self.selected_entry() {
                    if entry.location == EntryLocation::InFile {
                        self.mode = ViewMode::Confirming(ConfirmAction::Move(entry.key));
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(entry) = self.selected_entry() {
                    match copy_value(&entry) {
                        Ok(()) => self.notify("Value copied", NotificationLevel::Success),
                        Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                    }
                }
            }
            KeyCode::Char('K') => {
                if let Some(entry) = self.selected_entry() {
                    match crate::ops::copy_key(&entry) {
                        Ok(()) => self.notify("Key copied", NotificationLevel::Success),
                        Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                    }
                }
            }
            KeyCode::Char('C') => {
                if let Some(entry) = self.selected_entry() {
                    match copy_key_value(&entry) {
                        Ok(()) => self.notify("KEY=VALUE copied", NotificationLevel::Success),
                        Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                    }
                }
            }
            KeyCode::Char('v') => {
                // Toggle reveal for the selected entry's KEY (L9).
                if let Some(entry) = self.selected_entry() {
                    let k = entry.key;
                    if self.revealed.contains(&k) {
                        self.revealed.remove(&k);
                    } else {
                        self.revealed.insert(k);
                    }
                }
            }
            KeyCode::Char('L') => {
                // Show lifecycle info for the selected key
                if let Some(entry) = self.selected_entry() {
                    match crate::ops::lifecycle::orchestrator::get_state(&entry.key) {
                        Ok(state) => {
                            self.lifecycle_info = format!(
                                "{}: {:?} | rotations: {} | age: unknown",
                                entry.key, state, "n/a"
                            );
                            self.notify(&self.lifecycle_info.clone(), NotificationLevel::Success);
                        }
                        Err(e) => {
                            self.notify(
                                &format!("lifecycle error: {e}"),
                                NotificationLevel::Warning,
                            );
                        }
                    }
                } else {
                    self.notify("No key selected", NotificationLevel::Success);
                }
            }
            KeyCode::Char('/') => {
                self.search_query.clear();
                self.mode = ViewMode::Searching;
            }
            KeyCode::Char('S') | KeyCode::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.code == KeyCode::Char('S') =>
            {
                if self.has_unsaved_changes {
                    self.prepare_diff_preview();
                    self.mode = ViewMode::Confirming(ConfirmAction::Save);
                } else {
                    self.notify("No unsaved changes", NotificationLevel::Warning);
                }
            }
            KeyCode::Char('?') => {
                self.help_page = 0;
                self.mode = ViewMode::Help;
            }
            KeyCode::Char('r') => {
                if let Some(entry) = self.selected_entry() {
                    if entry.location == EntryLocation::Commented {
                        let source = entry.source_file.clone();
                        let key_name = entry.key;
                        if let Some(fi) = self.shell_files.iter().position(|sf| sf.path == source) {
                            self.snapshot(fi, &format!("Restore {}", key_name));
                            match undo_delete(&mut self.shell_files[fi], &key_name) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify("Restored", NotificationLevel::Success);
                                }
                                Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                            }
                        }
                    }
                }
            }
            KeyCode::Char('u') => {
                if let Some(undo_entry) = self.undo_stack.pop() {
                    if let Some(sf) = self.shell_files.get_mut(undo_entry.file_index) {
                        sf.lines = undo_entry.lines_snapshot;
                        self.refresh_entries();
                        self.notify(
                            &format!("Undone: {}", undo_entry.description),
                            NotificationLevel::Success,
                        );
                        if self.undo_stack.is_empty() {
                            self.has_unsaved_changes = false;
                        }
                    }
                } else {
                    self.notify("Nothing to undo", NotificationLevel::Warning);
                }
            }
            KeyCode::Char('I') => {
                self.input = TextInput::new("~/.env");
                self.mode = ViewMode::Importing;
            }
            KeyCode::Char('E') => {
                self.input = TextInput::new("~/.env.export");
                self.mode = ViewMode::Exporting;
            }
            KeyCode::Char('P') => {
                // Open profile selector
                let names = self.config.profiles.profile_names();
                let current_idx = names
                    .iter()
                    .position(|n| *n == self.config.profiles.active)
                    .unwrap_or(0);
                self.mode = ViewMode::ProfileSelector(current_idx);
            }
            KeyCode::Char('F') => {
                // Toggle AI tool fence for the current project
                self.toggle_fence();
            }
            KeyCode::Tab => {}
            _ => {}
        }
    }

    fn handle_profile_selector_key(&mut self, key: KeyEvent, selected_idx: usize) {
        let names = self.config.profiles.profile_names();
        let count = names.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down if selected_idx < count.saturating_sub(1) => {
                self.mode = ViewMode::ProfileSelector(selected_idx + 1);
            }
            KeyCode::Char('k') | KeyCode::Up if selected_idx > 0 => {
                self.mode = ViewMode::ProfileSelector(selected_idx - 1);
            }
            KeyCode::Enter => {
                if let Some(name) = names.get(selected_idx) {
                    let name = name.clone();
                    if name != self.config.profiles.active {
                        if let Some(sf) = self.shell_files.first_mut() {
                            match switch_profile(&mut self.config, sf, &name) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    // Reload entries with new profile
                                    self.entries = load_profile_entries(&self.config, sf);
                                    self.duplicate_keys = duplicate_key_set(&self.shell_files);
                                    self.notify(
                                        &format!("Switched to profile: {}", name),
                                        NotificationLevel::Success,
                                    );
                                }
                                Err(e) => {
                                    self.notify(&e.to_string(), NotificationLevel::Error);
                                }
                            }
                        }
                    }
                }
                self.mode = ViewMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = ViewMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_first_run_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('1') | KeyCode::Enter => {
                // Quick protect: create fence
                let cwd = std::env::current_dir().unwrap_or_default();
                match crate::ops::fence::create_fence(&cwd, false) {
                    Ok(result) => {
                        self.fence_enabled = true;
                        let count = result.files_created.len() + result.files_updated.len();
                        self.notify(
                            &format!("Fence created: {} file(s) protected", count),
                            NotificationLevel::Success,
                        );
                    }
                    Err(e) => {
                        self.notify(
                            &format!("Fence creation failed: {}", e),
                            NotificationLevel::Error,
                        );
                    }
                }
                self.first_run_completed = true;
                self.mode = ViewMode::Normal;
            }
            KeyCode::Char('2') => {
                self.first_run_completed = true;
                self.mode = ViewMode::Normal;
                self.notify(
                    "First-run skipped. Run 'envforge fence' to protect later.",
                    NotificationLevel::Success,
                );
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.first_run_completed = true;
                self.mode = ViewMode::Normal;
            }
            _ => {}
        }
    }

    fn toggle_fence(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        // Refresh the resolved target summary after any config-touching toggle (FR16).
        self.refresh_fence_resolved_targets();
        if self.fence_enabled {
            match crate::ops::fence::remove_fence(&cwd, false) {
                Ok(result) => {
                    self.fence_enabled = false;
                    let count = result.files_removed.len() + result.files_updated.len();
                    self.notify(
                        &format!("Fence removed: {} file(s) restored", count),
                        NotificationLevel::Success,
                    );
                }
                Err(e) => {
                    self.notify(
                        &format!("Fence removal failed: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            match crate::ops::fence::create_fence(&cwd, false) {
                Ok(result) => {
                    self.fence_enabled = true;
                    let count = result.files_created.len() + result.files_updated.len();
                    self.notify(
                        &format!("Fence created: {} file(s) protected", count),
                        NotificationLevel::Success,
                    );
                }
                Err(e) => {
                    self.notify(
                        &format!("Fence creation failed: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Reload the resolved fence target list from config.
    ///
    /// Called after operations that may change config (toggle, first-run).
    /// On any error the resolved list is reset to the all-enabled default.
    pub fn refresh_fence_resolved_targets(&mut self) {
        let fence_cfg = crate::config::load_or_create_default()
            .map(|c| c.fence)
            .unwrap_or_default();
        self.fence_resolved_targets = crate::ops::fence::resolve_fence_targets(&fence_cfg);
    }

    fn handle_import_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let path_str = self.input.value().to_string();
                let path = shellexpand_path(&path_str);
                if path.exists() {
                    match parse_dotenv(&path) {
                        Ok(entries) => {
                            if entries.is_empty() {
                                self.notify("No entries in file", NotificationLevel::Warning);
                            } else {
                                self.snapshot(0, &format!("Import from {}", path_str));
                                if let Some(sf) = self.shell_files.first_mut() {
                                    let result = import_entries(sf, &entries, &self.config, false);
                                    self.has_unsaved_changes =
                                        result.added > 0 || result.overwritten > 0;
                                    self.refresh_entries();
                                    self.notify(
                                        &format!(
                                            "Imported: {} added, {} skipped",
                                            result.added, result.skipped
                                        ),
                                        NotificationLevel::Success,
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            self.notify(&format!("Import error: {}", e), NotificationLevel::Error)
                        }
                    }
                } else {
                    self.notify(
                        &format!("File not found: {}", path_str),
                        NotificationLevel::Error,
                    );
                }
                self.mode = ViewMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = ViewMode::Normal;
            }
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Char(c) => self.input.insert(c),
            _ => {}
        }
    }

    fn handle_export_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let path_str = self.input.value().to_string();
                let output = export_entries(&self.entries, false, None);
                if path_str.is_empty() || path_str == "-" {
                    self.notify("Export requires a file path", NotificationLevel::Warning);
                } else {
                    let path = shellexpand_path(&path_str);
                    match std::fs::write(&path, &output) {
                        Ok(()) => {
                            self.notify(
                                &format!("Exported {} entries to {}", self.entries.len(), path_str),
                                NotificationLevel::Success,
                            );
                        }
                        Err(e) => {
                            self.notify(&format!("Export error: {}", e), NotificationLevel::Error)
                        }
                    }
                }
                self.mode = ViewMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = ViewMode::Normal;
            }
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Char(c) => self.input.insert(c),
            _ => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let visible = self.visible_entries();
                if let Some(entry) = visible.get(self.selected) {
                    let new_value = self.input.value().to_string();
                    let key_name = entry.key.clone();
                    let source = entry.source_file.clone();
                    if let Some(fi) = self.shell_files.iter().position(|sf| sf.path == source) {
                        self.snapshot(fi, &format!("Edit {}", key_name));
                        match edit_entry(&mut self.shell_files[fi], &key_name, &new_value) {
                            Ok(()) => {
                                self.has_unsaved_changes = true;
                                self.refresh_entries();
                                self.notify("Edited (unsaved)", NotificationLevel::Success);
                            }
                            Err(e) => {
                                self.undo_stack.pop(); // revert snapshot on error
                                self.notify(&e.to_string(), NotificationLevel::Error);
                            }
                        }
                    }
                }
                self.mode = ViewMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = ViewMode::Normal;
            }
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Home => self.input.move_home(),
            KeyCode::End => self.input.move_end(),
            KeyCode::Char(c) => self.input.insert(c),
            _ => {}
        }
    }

    fn handle_add_key(&mut self, key: KeyEvent, field: AddField) {
        match key.code {
            KeyCode::Enter => {
                if field == AddField::Key {
                    self.mode = ViewMode::Adding(AddField::Value);
                } else {
                    // Commit add to target file (profile or shared)
                    let key_str = self.add_key_input.value().to_string();
                    let value_str = self.add_value_input.value().to_string();
                    if !key_str.is_empty() {
                        let fi = self.target_file_index();
                        let target_name = match self.add_target {
                            AddTarget::Profile => self.config.profiles.active.clone(),
                            AddTarget::Shared => "shared".to_string(),
                        };
                        if let Some(sf) = self.shell_files.get_mut(fi) {
                            self.undo_stack.push(
                                fi,
                                &sf.lines,
                                &format!("Add {} to {}", key_str, target_name),
                            );
                            match add_entry(
                                sf,
                                &key_str,
                                &value_str,
                                ExportStyle::Export,
                                QuoteStyle::Double,
                                0,
                                0,
                            ) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify(
                                        &format!("Added to {} (unsaved)", target_name),
                                        NotificationLevel::Success,
                                    );
                                }
                                Err(e) => {
                                    self.undo_stack.pop();
                                    self.notify(&e.to_string(), NotificationLevel::Error);
                                }
                            }
                        }
                    }
                    self.mode = ViewMode::Normal;
                }
            }
            KeyCode::Tab => {
                // Ctrl+Tab or just Tab to switch field
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Toggle target: profile ↔ shared
                    self.add_target = match self.add_target {
                        AddTarget::Profile => AddTarget::Shared,
                        AddTarget::Shared => AddTarget::Profile,
                    };
                } else {
                    self.mode = ViewMode::Adding(match field {
                        AddField::Key => AddField::Value,
                        AddField::Value => AddField::Key,
                    });
                }
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+T to toggle target: profile ↔ shared
                self.add_target = match self.add_target {
                    AddTarget::Profile => AddTarget::Shared,
                    AddTarget::Shared => AddTarget::Profile,
                };
            }
            KeyCode::Esc => {
                self.add_target = AddTarget::Profile; // Reset target
                self.mode = ViewMode::Normal;
            }
            KeyCode::Backspace => match field {
                AddField::Key => self.add_key_input.backspace(),
                AddField::Value => self.add_value_input.backspace(),
            },
            KeyCode::Char(c) => match field {
                AddField::Key => self.add_key_input.insert(c),
                AddField::Value => self.add_value_input.insert(c),
            },
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.selected = 0;
                self.mode = ViewMode::Normal;
            }
            KeyCode::Enter => {
                self.mode = ViewMode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.selected = 0;
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.selected = 0;
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent, action: ConfirmAction) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                match action {
                    ConfirmAction::Delete(ref key_name) => {
                        let key_name = key_name.clone();
                        self.snapshot(0, &format!("Delete {}", key_name));
                        if let Some(sf) = self.shell_files.first_mut() {
                            match soft_delete(sf, &key_name) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify("Deleted (unsaved)", NotificationLevel::Success);
                                }
                                Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                            }
                        }
                    }
                    ConfirmAction::Move(ref key_name) => {
                        let key_name = key_name.clone();
                        let ref_path = shellexpand_path(&self.config.files.reference);
                        if self.shell_files.len() >= 2 {
                            let (first, rest) = self.shell_files.split_at_mut(1);
                            match move_to_reference(
                                &mut first[0],
                                &mut rest[0],
                                &key_name,
                                &ref_path,
                            ) {
                                Ok(()) => {
                                    self.has_unsaved_changes = true;
                                    self.refresh_entries();
                                    self.notify("Moved (unsaved)", NotificationLevel::Success);
                                }
                                Err(e) => self.notify(&e.to_string(), NotificationLevel::Error),
                            }
                        }
                    }
                    ConfirmAction::Save => {
                        self.save_all();
                    }
                    ConfirmAction::Quit => {
                        self.should_quit = true;
                    }
                }
                self.mode = ViewMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = ViewMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_diff_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = ViewMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = ViewMode::Normal;
            }
            KeyCode::Tab | KeyCode::Right => {
                self.help_page = (self.help_page + 1) % 3;
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.help_page = (self.help_page + 2) % 3;
            }
            KeyCode::Char('1') => {
                self.help_page = 0;
            }
            KeyCode::Char('2') => {
                self.help_page = 1;
            }
            KeyCode::Char('3') => {
                self.help_page = 2;
            }
            _ => {}
        }
    }

    fn prepare_diff_preview(&mut self) {
        let mut diffs = String::new();
        for sf in &self.shell_files {
            let diff = generate_diff_from_strings(
                &std::fs::read_to_string(&sf.path).unwrap_or_default(),
                &serialize_shell_file(sf),
                &sf.path.to_string_lossy(),
            );
            if !diff.is_empty() {
                diffs.push_str(&diff);
                diffs.push('\n');
            }
        }
        // H2/FR1: redact sensitive cleartext values (old AND new sides) before
        // the diff is stored in App state or rendered — the diff preview is the
        // one screen that would otherwise show a secret in full.
        let diffs = crate::ops::sanitize::redact_sensitive_assignments(&diffs);
        self.diff_content = if diffs.is_empty() {
            "No changes to preview.".to_string()
        } else {
            diffs
        };
    }

    fn save_all(&mut self) {
        for sf in &self.shell_files {
            let content = serialize_shell_file(sf);
            match safe_write(&sf.path, &content, Some(sf.hash)) {
                Ok(()) => {}
                Err(e) => {
                    self.notify(&format!("Save failed: {}", e), NotificationLevel::Error);
                    return;
                }
            }
        }
        // Re-parse to update hashes
        let paths: Vec<_> = self.shell_files.iter().map(|sf| sf.path.clone()).collect();
        self.shell_files.clear();
        for path in paths {
            if let Ok(sf) = parse_shell_file(&path) {
                self.shell_files.push(sf);
            }
        }
        self.refresh_entries();
        self.has_unsaved_changes = false;
        self.undo_stack.clear();
        self.notify("Saved", NotificationLevel::Success);
    }
}

/// Run the TUI application.
pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // H3/FR8: guarantee terminal restore on ANY exit path — normal return,
    // `?` early-return, or panic — so a revealed secret can never be stranded
    // on a raw/alt screen. The panic hook restores BEFORE the default panic
    // message prints (otherwise it renders into a raw terminal); the RAII
    // guard covers the non-panic paths.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));
    let _tui_guard = TuiGuard;

    // Main loop
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| render::render(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                    app.handle_key(key);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    // Terminal restore is handled by `_tui_guard` (Drop) — covers normal,
    // `?`, and panic paths uniformly.
    Ok(())
}

/// Restore the terminal to a sane state: leave raw mode + alternate screen and
/// re-enable the cursor. Best-effort and idempotent — called from both the
/// Drop guard and the panic hook, so it must never panic or early-return.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    );
}

/// RAII guard that restores the terminal when `run_tui` returns by any path.
struct TuiGuard;

impl Drop for TuiGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Expand ~ in path strings.
fn shellexpand_path(path_str: &str) -> std::path::PathBuf {
    if let Some(rest) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path_str)
}
