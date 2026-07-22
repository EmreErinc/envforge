use envforge::ui::{AddField, App, ViewMode};

#[test]
fn test_return_mode_initialization() {
    let mut app = App::new().expect("failed to create App");
    assert!(app.return_mode.is_none());

    app.open_secret_generator_with_return(ViewMode::Editing);
    assert_eq!(app.return_mode, Some(ViewMode::Editing));
    assert_eq!(app.mode, ViewMode::SecretGenerator);
}

#[test]
fn test_modal_chaining_editing_apply() {
    let mut app = App::new().expect("failed to create App");
    app.open_secret_generator_with_return(ViewMode::Editing);
    app.generated_secret = "secret123".to_string();
    app.handle_key(crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter));

    assert_eq!(app.mode, ViewMode::Editing);
    assert_eq!(app.input.value(), "secret123");
}

#[test]
fn test_modal_chaining_adding_apply() {
    let mut app = App::new().expect("failed to create App");
    app.open_secret_generator_with_return(ViewMode::Adding(AddField::Key));
    app.generated_secret = "secret456".to_string();
    app.handle_key(crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter));

    assert_eq!(app.mode, ViewMode::Adding(AddField::Value));
    assert_eq!(app.add_value_input.value(), "secret456");
}
