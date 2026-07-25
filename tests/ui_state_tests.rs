use envforge::ui::secret_gen::{generate_secret, SecretGenFormat, SecretGenOpts};

#[test]
fn test_generate_secret_formats() {
    let opts_alpha = SecretGenOpts {
        format: SecretGenFormat::AlphaNumericOnly,
        length: 32,
    };
    let secret = generate_secret(&opts_alpha);
    assert_eq!(secret.len(), 32);
    assert!(secret.chars().all(|c| c.is_alphanumeric()));

    let opts_uuid = SecretGenOpts {
        format: SecretGenFormat::UuidV4,
        length: 36,
    };
    let uuid_str = generate_secret(&opts_uuid);
    assert!(uuid::Uuid::parse_str(&uuid_str).is_ok());
}

#[test]
fn test_mac_option_and_ctrl_profile_shortcuts() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use envforge::ui::App;

    let mut app = App::new().expect("failed to create App");
    // Test Ctrl+1
    let ctrl1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL);
    app.handle_key(ctrl1);

    // Test macOS Option+1 ('¡')
    let opt1 = KeyEvent::new(KeyCode::Char('¡'), KeyModifiers::NONE);
    app.handle_key(opt1);
}

#[test]
fn test_profile_selector_and_palette_parity() {
    use envforge::ui::App;
    let app = App::new().expect("failed to create App");
    let profiles = app.config.profiles.profile_names();
    println!("Discovered profiles: {:?}", profiles);
    assert!(profiles.contains(&"default".to_string()));
}

#[test]
fn test_health_and_matrix_view_modes() {
    use envforge::ui::{App, ViewMode};

    let mut app = App::new().expect("failed to create app");
    assert_eq!(app.mode, ViewMode::Normal);

    app.mode = ViewMode::HealthAudit;
    assert_eq!(app.mode, ViewMode::HealthAudit);

    app.mode = ViewMode::ProfileMatrix;
    assert_eq!(app.mode, ViewMode::ProfileMatrix);
}

#[test]
fn test_health_and_matrix_keybindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use envforge::ui::{App, ViewMode};

    let mut app = App::new().expect("failed to create app");

    // Press H
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE));
    assert_eq!(app.mode, ViewMode::HealthAudit);

    // Press Esc to exit
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, ViewMode::Normal);

    // Press M
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::NONE));
    assert_eq!(app.mode, ViewMode::ProfileMatrix);

    // Press q to exit
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert_eq!(app.mode, ViewMode::Normal);
}

#[test]
fn test_render_health_audit_mode() {
    use envforge::ui::{App, HealthIssue, HealthSeverity, ViewMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::new().expect("failed to create app");
    app.mode = ViewMode::HealthAudit;
    app.health_report.issues.push(HealthIssue {
        key: "TEST_KEY".into(),
        severity: HealthSeverity::Warning,
        message: "Duplicate key definition".into(),
    });
    app.health_report.warning_count = 1;

    assert_eq!(app.health_report.issues.len(), 1);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            envforge::ui::render::render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);
    assert!(content.contains("Environment & Schema Health Audit (H)"));
    assert!(content.contains("TEST_KEY"));
    assert!(content.contains("[WARNING]"));
}

#[test]
fn test_matrix_view_scaffolding() {
    use envforge::ui::{App, MatrixCellStatus, ProfileMatrixRow, ViewMode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::new().expect("failed to create app");
    app.mode = ViewMode::ProfileMatrix;
    app.matrix_data.rows.push(ProfileMatrixRow {
        key: "DB_PORT".into(),
        shared_status: MatrixCellStatus::Set("5432".into()),
        profile_statuses: vec![("dev".into(), MatrixCellStatus::Missing)],
    });
    app.matrix_data.profiles = vec!["dev".into()];

    assert_eq!(app.matrix_data.rows.len(), 1);

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            envforge::ui::render::render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);
    assert!(content.contains("Profile Matrix & Multi-Environment Grid (M)"));
    assert!(content.contains("DB_PORT"));
    assert!(content.contains("[SET] 5432"));
    assert!(content.contains("[MISSING]"));
    assert!(content.contains("Navigate Grid"));
}
