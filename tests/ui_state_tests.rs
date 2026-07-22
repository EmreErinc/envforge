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
