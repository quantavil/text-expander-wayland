use std::sync::Arc;
use text_expander::config::{Trigger, Var, VarParams};

#[test]
fn test_trigger_expand_simple() {
    let trigger = Trigger {
        replace: "hello world".to_string(),
        vars: Arc::new(vec![]),
    };
    assert_eq!(trigger.expand(), "hello world");
}

#[test]
fn test_trigger_expand_echo() {
    let trigger = Trigger {
        replace: "hello {{name}}".to_string(),
        vars: Arc::new(vec![Var {
            name: "name".to_string(),
            var_type: "echo".to_string(),
            params: VarParams {
                echo: Some("Rust".to_string()),
                format: None,
                cmd: None,
            },
        }]),
    };
    assert_eq!(trigger.expand(), "hello Rust");
}

#[test]
fn test_trigger_expand_date_default() {
    let trigger = Trigger {
        replace: "today is {{date}}".to_string(),
        vars: Arc::new(vec![Var {
            name: "date".to_string(),
            var_type: "date".to_string(),
            params: VarParams::default(),
        }]),
    };
    let expanded = trigger.expand();
    assert!(expanded.starts_with("today is "));
    // Default format is %Y-%m-%d (10 characters)
    assert_eq!(expanded.len(), "today is ".len() + 10);
}

#[test]
fn test_trigger_expand_date_custom_format() {
    let trigger = Trigger {
        replace: "year is {{year}}".to_string(),
        vars: Arc::new(vec![Var {
            name: "year".to_string(),
            var_type: "date".to_string(),
            params: VarParams {
                format: Some("%Y".to_string()),
                echo: None,
                cmd: None,
            },
        }]),
    };
    let expanded = trigger.expand();
    let current_year = chrono::Local::now().format("%Y").to_string();
    assert_eq!(expanded, format!("year is {}", current_year));
}

#[test]
fn test_trigger_expand_unknown_var_type() {
    let trigger = Trigger {
        replace: "hello {{unknown}}".to_string(),
        vars: Arc::new(vec![Var {
            name: "unknown".to_string(),
            var_type: "invalid_type".to_string(),
            params: VarParams::default(),
        }]),
    };
    // If the type is unknown, it should fallback to placeholder itself
    assert_eq!(trigger.expand(), "hello {{unknown}}");
}
