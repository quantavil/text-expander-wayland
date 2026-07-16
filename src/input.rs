use evdev::{Device, EventType, KeyCode};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};
use crate::config::Trigger;

pub fn key_to_char(key: KeyCode, shift: bool) -> Option<char> {
    let c = match key {
        KeyCode::KEY_A => 'a', KeyCode::KEY_B => 'b', KeyCode::KEY_C => 'c', KeyCode::KEY_D => 'd',
        KeyCode::KEY_E => 'e', KeyCode::KEY_F => 'f', KeyCode::KEY_G => 'g', KeyCode::KEY_H => 'h',
        KeyCode::KEY_I => 'i', KeyCode::KEY_J => 'j', KeyCode::KEY_K => 'k', KeyCode::KEY_L => 'l',
        KeyCode::KEY_M => 'm', KeyCode::KEY_N => 'n', KeyCode::KEY_O => 'o', KeyCode::KEY_P => 'p',
        KeyCode::KEY_Q => 'q', KeyCode::KEY_R => 'r', KeyCode::KEY_S => 's', KeyCode::KEY_T => 't',
        KeyCode::KEY_U => 'u', KeyCode::KEY_V => 'v', KeyCode::KEY_W => 'w', KeyCode::KEY_X => 'x',
        KeyCode::KEY_Y => 'y', KeyCode::KEY_Z => 'z',
        KeyCode::KEY_1 => if shift { '!' } else { '1' },
        KeyCode::KEY_2 => if shift { '@' } else { '2' },
        KeyCode::KEY_3 => if shift { '#' } else { '3' },
        KeyCode::KEY_4 => if shift { '$' } else { '4' },
        KeyCode::KEY_5 => if shift { '%' } else { '5' },
        KeyCode::KEY_6 => if shift { '^' } else { '6' },
        KeyCode::KEY_7 => if shift { '&' } else { '7' },
        KeyCode::KEY_8 => if shift { '*' } else { '8' },
        KeyCode::KEY_9 => if shift { '(' } else { '9' },
        KeyCode::KEY_0 => if shift { ')' } else { '0' },
        KeyCode::KEY_MINUS => if shift { '_' } else { '-' },
        KeyCode::KEY_EQUAL => if shift { '+' } else { '=' },
        KeyCode::KEY_LEFTBRACE => if shift { '{' } else { '[' },
        KeyCode::KEY_RIGHTBRACE => if shift { '}' } else { ']' },
        KeyCode::KEY_SEMICOLON => if shift { ':' } else { ';' },
        KeyCode::KEY_APOSTROPHE => if shift { '"' } else { '\'' },
        KeyCode::KEY_GRAVE => if shift { '~' } else { '`' },
        KeyCode::KEY_BACKSLASH => if shift { '|' } else { '\\' },
        KeyCode::KEY_COMMA => if shift { '<' } else { ',' },
        KeyCode::KEY_DOT => if shift { '>' } else { '.' },
        KeyCode::KEY_SLASH => if shift { '?' } else { '/' },
        KeyCode::KEY_SPACE => ' ',
        _ => return None,
    };
    Some(if shift && c.is_ascii_alphabetic() { c.to_ascii_uppercase() } else { c })
}

pub fn find_keyboards() -> Vec<(PathBuf, Device)> {
    let mut keyboards = Vec::new();
    let mut virtual_kbd = None;

    let Ok(entries) = fs::read_dir("/dev/input") else { return keyboards };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.to_string_lossy().contains("event") { continue }

        let Ok(device) = Device::open(&path) else { continue };

        if !device.supported_events().contains(EventType::KEY) { continue }

        let Some(keys) = device.supported_keys() else { continue };
        if !keys.contains(KeyCode::KEY_A) || !keys.contains(KeyCode::KEY_Z) { continue }

        let name = device.name().unwrap_or("unknown");
        eprintln!("\x1b[34m⌨️  [input]\x1b[0m Found keyboard: {:?} - {}", path, name);

        let name_lower = name.to_lowercase();
        let is_remapper = name_lower.contains("keyd") || name_lower.contains("kmonad") || name_lower.contains("kanata");

        if is_remapper {
            virtual_kbd = Some((path, device));
        } else if !name_lower.contains("virtual") {
            keyboards.push((path, device));
        }
    }

    if let Some(vkbd) = virtual_kbd {
        eprintln!("\x1b[35m🔒 [input]\x1b[0m Using virtual keyboard only (keyd/kmonad/kanata detected)");
        vec![vkbd]
    } else {
        keyboards
    }
}

pub struct TextExpander {
    sorted_triggers: Vec<(String, Trigger)>,
    buffer: String,
    max_len: usize,
    shift: bool,
    capslock: bool,
}

impl TextExpander {
    pub fn new(triggers: HashMap<String, Trigger>) -> Self {
        let max_len = triggers.keys().map(|k| k.len()).max().unwrap_or(64);
        let mut sorted_triggers: Vec<(String, Trigger)> = triggers.into_iter().collect();
        // Sort by length descending to make trigger matching deterministic (longest match wins)
        sorted_triggers.sort_by(|a, b| {
            b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0))
        });
        Self { sorted_triggers, buffer: String::with_capacity(max_len + 1), max_len, shift: false, capslock: false }
    }

    pub fn process(&mut self, key: KeyCode, pressed: bool) -> Option<(usize, Trigger)> {
        if key == KeyCode::KEY_LEFTSHIFT || key == KeyCode::KEY_RIGHTSHIFT {
            self.shift = pressed;
            return None;
        }

        if key == KeyCode::KEY_CAPSLOCK && pressed {
            self.capslock = !self.capslock;
            return None;
        }

        if !pressed { return None }

        match key {
            KeyCode::KEY_ENTER | KeyCode::KEY_TAB | KeyCode::KEY_ESC => {
                self.buffer.clear();
                return None;
            }
            KeyCode::KEY_BACKSPACE => {
                self.buffer.pop();
                return None;
            }
            _ => {}
        }

        // Caps Lock only inverts the shift state for alphabetic keys.
        let is_alphabetic = matches!(
            key,
            KeyCode::KEY_A | KeyCode::KEY_B | KeyCode::KEY_C | KeyCode::KEY_D |
            KeyCode::KEY_E | KeyCode::KEY_F | KeyCode::KEY_G | KeyCode::KEY_H |
            KeyCode::KEY_I | KeyCode::KEY_J | KeyCode::KEY_K | KeyCode::KEY_L |
            KeyCode::KEY_M | KeyCode::KEY_N | KeyCode::KEY_O | KeyCode::KEY_P |
            KeyCode::KEY_Q | KeyCode::KEY_R | KeyCode::KEY_S | KeyCode::KEY_T |
            KeyCode::KEY_U | KeyCode::KEY_V | KeyCode::KEY_W | KeyCode::KEY_X |
            KeyCode::KEY_Y | KeyCode::KEY_Z
        );

        let effective_shift = if is_alphabetic {
            self.shift ^ self.capslock
        } else {
            self.shift
        };

        if let Some(c) = key_to_char(key, effective_shift) {
            self.buffer.push(c);
            if self.buffer.len() > self.max_len {
                self.buffer.drain(..self.buffer.len() - self.max_len);
            }

            for (trig, data) in &self.sorted_triggers {
                if self.buffer.ends_with(trig) {
                    // Word boundary check: if trigger starts with alphanumeric, preceding char must not be alphanumeric
                    if let Some(first_char) = trig.chars().next() {
                        if first_char.is_alphanumeric() {
                            let start_idx = self.buffer.len() - trig.len();
                            if start_idx > 0 {
                                if let Some(char_before) = self.buffer.chars().nth(start_idx - 1) {
                                    if char_before.is_alphanumeric() {
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    let result = (trig.len(), data.clone());
                    self.buffer.clear();
                    return Some(result);
                }
            }
        }
        None
    }
}
