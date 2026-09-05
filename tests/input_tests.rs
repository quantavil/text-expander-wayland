use std::collections::HashMap;
use evdev::KeyCode;
use text_expander::input::{TextExpander, InputEvent};
use text_expander::config::Trigger;

#[test]
fn test_process_expansion() {
    let mut triggers = HashMap::new();
    triggers.insert(";ip".to_string(), Trigger {
        replace: "127.0.0.1".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    triggers.insert(";date".to_string(), Trigger {
        replace: "2026-07-16".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });

    let mut expander = TextExpander::new(triggers, None, false);

    assert!(expander.process(KeyCode::KEY_A, true).is_none());
    assert!(expander.process(KeyCode::KEY_A, false).is_none());

    assert!(expander.process(KeyCode::KEY_SEMICOLON, true).is_none());
    assert!(expander.process(KeyCode::KEY_SEMICOLON, false).is_none());
    assert!(expander.process(KeyCode::KEY_I, true).is_none());
    assert!(expander.process(KeyCode::KEY_I, false).is_none());
    
    let ev = expander.process(KeyCode::KEY_P, true);
    assert!(ev.is_some());
    if let Some(InputEvent::Expansion(len, trig)) = ev {
        assert_eq!(len, 3);
        assert_eq!(trig.replace, "127.0.0.1");
    } else {
        panic!("Expected expansion event");
    }
}

#[test]
fn test_word_boundary_longest_trigger() {
    let mut triggers = HashMap::new();
    triggers.insert("ip".to_string(), Trigger {
        replace: "127.0.0.1".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    let mut expander = TextExpander::new(triggers, None, false);

    assert!(expander.process(KeyCode::KEY_A, true).is_none());
    assert!(expander.process(KeyCode::KEY_A, false).is_none());

    assert!(expander.process(KeyCode::KEY_I, true).is_none());
    assert!(expander.process(KeyCode::KEY_I, false).is_none());

    let ev = expander.process(KeyCode::KEY_P, true);
    assert!(ev.is_none());
}

#[test]
fn test_modifier_pollution() {
    let mut triggers = HashMap::new();
    triggers.insert("v".to_string(), Trigger {
        replace: "paste".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    let mut expander = TextExpander::new(triggers, None, false);

    assert!(expander.process(KeyCode::KEY_LEFTCTRL, true).is_none());

    let ev = expander.process(KeyCode::KEY_V, true);
    assert!(ev.is_none());
    assert!(expander.process(KeyCode::KEY_V, false).is_none());

    assert!(expander.process(KeyCode::KEY_LEFTCTRL, false).is_none());

    let ev = expander.process(KeyCode::KEY_V, true);
    assert!(ev.is_some());
}

#[test]
fn test_hotkey_parse_valid() {
    use text_expander::input::Hotkey;

    let hk = Hotkey::parse("ctrl+alt+f").unwrap();
    assert!(hk.ctrl);
    assert!(hk.alt);
    assert!(!hk.shift);
    assert!(!hk.meta);
    assert_eq!(hk.key, KeyCode::KEY_F);

    let hk2 = Hotkey::parse("super+space").unwrap();
    assert!(!hk2.ctrl);
    assert!(!hk2.alt);
    assert!(!hk2.shift);
    assert!(hk2.meta);
    assert_eq!(hk2.key, KeyCode::KEY_SPACE);
}

#[test]
fn test_hotkey_parse_invalid() {
    use text_expander::input::Hotkey;

    // No actual key code
    assert!(Hotkey::parse("ctrl+alt").is_none());
    // Unknown modifiers should be ignored but if there's no valid key, it fails
    assert!(Hotkey::parse("invalid_mod").is_none());
}

#[test]
fn test_keypad_keys_mapped() {
    use text_expander::input::key_to_char;
    assert_eq!(key_to_char(KeyCode::KEY_KP1, false), Some('1'));
    assert_eq!(key_to_char(KeyCode::KEY_KPPLUS, false), Some('+'));
}

#[test]
fn test_paired_modifiers_do_not_desync() {
    let mut triggers = HashMap::new();
    triggers.insert("A".to_string(), Trigger {
        replace: "success".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    let mut expander = TextExpander::new(triggers, None, false);

    assert!(expander.process(KeyCode::KEY_LEFTSHIFT, true).is_none());
    assert!(expander.process(KeyCode::KEY_RIGHTSHIFT, true).is_none());
    assert!(expander.process(KeyCode::KEY_LEFTSHIFT, false).is_none());

    let ev = expander.process(KeyCode::KEY_A, true);
    assert!(ev.is_some(), "Trigger 'A' should match because Right Shift is still held");

    assert!(expander.process(KeyCode::KEY_RIGHTSHIFT, false).is_none());
}

#[test]
fn test_capslock_repeat_does_not_toggle() {
    let mut triggers = HashMap::new();
    triggers.insert("A".to_string(), Trigger {
        replace: "caps_active".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    let mut expander = TextExpander::new(triggers, None, false);

    assert!(expander.process(KeyCode::KEY_CAPSLOCK, true).is_none());
    assert!(expander.process(KeyCode::KEY_CAPSLOCK, true).is_none());
    assert!(expander.process(KeyCode::KEY_CAPSLOCK, true).is_none());

    let ev = expander.process(KeyCode::KEY_A, true);
    assert!(ev.is_some(), "CapsLock should remain ON after repeated key events");

    assert!(expander.process(KeyCode::KEY_CAPSLOCK, false).is_none());
}

#[test]
fn test_backspace_repeat_pops_buffer() {
    let mut triggers = HashMap::new();
    triggers.insert(";b".to_string(), Trigger {
        replace: "expanded".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });
    let mut expander = TextExpander::new(triggers, None, false);

    expander.process(KeyCode::KEY_SEMICOLON, true);
    expander.process(KeyCode::KEY_SEMICOLON, false);
    expander.process(KeyCode::KEY_X, true);
    expander.process(KeyCode::KEY_X, false);

    expander.process(KeyCode::KEY_BACKSPACE, true);
    expander.process(KeyCode::KEY_BACKSPACE, true);
    expander.process(KeyCode::KEY_BACKSPACE, false);

    let ev = expander.process(KeyCode::KEY_B, true);
    assert!(ev.is_none());
}

#[test]
fn test_expansion_char_count() {
    let mut triggers = HashMap::new();
    triggers.insert(";ts".to_string(), Trigger {
        replace: "now".to_string(),
        vars: std::sync::Arc::new(vec![]),
    });

    let mut expander = TextExpander::new(triggers, None, false);
    expander.process(KeyCode::KEY_SEMICOLON, true);
    expander.process(KeyCode::KEY_SEMICOLON, false);
    expander.process(KeyCode::KEY_T, true);
    expander.process(KeyCode::KEY_T, false);

    let ev = expander.process(KeyCode::KEY_S, true);
    assert!(ev.is_some());
    if let Some(InputEvent::Expansion(chars, trig)) = ev {
        assert_eq!(chars, 3);
        assert_eq!(trig.replace, "now");
    }
}



