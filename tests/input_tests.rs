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


