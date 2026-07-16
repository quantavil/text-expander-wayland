use evdev::KeyCode;
use text_expander::inject::has_key_conflict;

#[test]
fn test_has_key_conflict_none() {
    assert!(!has_key_conflict("hello", None));
}

#[test]
fn test_has_key_conflict_true() {
    // Conflict on key 'h' (lowercase/uppercase)
    assert!(has_key_conflict("hello", Some(KeyCode::KEY_H)));
    assert!(has_key_conflict("Hello", Some(KeyCode::KEY_H)));
}

#[test]
fn test_has_key_conflict_false() {
    // No conflict on key 'x'
    assert!(!has_key_conflict("hello", Some(KeyCode::KEY_X)));
}
