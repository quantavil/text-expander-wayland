use evdev::{Device, EventType, KeyCode};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU8, Ordering},
};
use crate::config::Trigger;

pub static MODIFIERS_DOWN: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: KeyCode,
}

impl Hotkey {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<String> = s.split('+').map(|p| p.trim().to_lowercase()).collect();
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut meta = false;
        let mut key = None;

        for part in &parts {
            match part.as_str() {
                "ctrl" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                "super" | "meta" | "win" => meta = true,
                other => {
                    if other.len() == 1 {
                        let c = other.chars().next().unwrap();
                        key = char_to_keycode(c);
                    } else if other == "space" {
                        key = Some(KeyCode::KEY_SPACE);
                    }
                }
            }
        }

        key.map(|k| Hotkey { ctrl, alt, shift, meta, key: k })
    }
}

const KEY_MAP: &[(KeyCode, char, char)] = &[
    (KeyCode::KEY_A, 'a', 'A'), (KeyCode::KEY_B, 'b', 'B'), (KeyCode::KEY_C, 'c', 'C'), (KeyCode::KEY_D, 'd', 'D'),
    (KeyCode::KEY_E, 'e', 'E'), (KeyCode::KEY_F, 'f', 'F'), (KeyCode::KEY_G, 'g', 'G'), (KeyCode::KEY_H, 'h', 'H'),
    (KeyCode::KEY_I, 'i', 'I'), (KeyCode::KEY_J, 'j', 'J'), (KeyCode::KEY_K, 'k', 'K'), (KeyCode::KEY_L, 'l', 'L'),
    (KeyCode::KEY_M, 'm', 'M'), (KeyCode::KEY_N, 'n', 'N'), (KeyCode::KEY_O, 'o', 'O'), (KeyCode::KEY_P, 'p', 'P'),
    (KeyCode::KEY_Q, 'q', 'Q'), (KeyCode::KEY_R, 'r', 'R'), (KeyCode::KEY_S, 's', 'S'), (KeyCode::KEY_T, 't', 'T'),
    (KeyCode::KEY_U, 'u', 'U'), (KeyCode::KEY_V, 'v', 'V'), (KeyCode::KEY_W, 'w', 'W'), (KeyCode::KEY_X, 'x', 'X'),
    (KeyCode::KEY_Y, 'y', 'Y'), (KeyCode::KEY_Z, 'z', 'Z'),
    (KeyCode::KEY_1, '1', '!'), (KeyCode::KEY_2, '2', '@'), (KeyCode::KEY_3, '3', '#'), (KeyCode::KEY_4, '4', '$'),
    (KeyCode::KEY_5, '5', '%'), (KeyCode::KEY_6, '6', '^'), (KeyCode::KEY_7, '7', '&'), (KeyCode::KEY_8, '8', '*'),
    (KeyCode::KEY_9, '9', '('), (KeyCode::KEY_0, '0', ')'),
    (KeyCode::KEY_MINUS, '-', '_'), (KeyCode::KEY_EQUAL, '=', '+'),
    (KeyCode::KEY_LEFTBRACE, '[', '{'), (KeyCode::KEY_RIGHTBRACE, ']', '}'),
    (KeyCode::KEY_SEMICOLON, ';', ':'), (KeyCode::KEY_APOSTROPHE, '\'', '"'),
    (KeyCode::KEY_GRAVE, '`', '~'), (KeyCode::KEY_BACKSLASH, '\\', '|'),
    (KeyCode::KEY_COMMA, ',', '<'), (KeyCode::KEY_DOT, '.', '>'), (KeyCode::KEY_SLASH, '/', '?'),
    (KeyCode::KEY_SPACE, ' ', ' '),
];

pub fn char_to_keycode(c: char) -> Option<KeyCode> {
    let lower_c = c.to_ascii_lowercase();
    KEY_MAP.iter().find(|&&(_, normal, shifted)| {
        normal.to_ascii_lowercase() == lower_c || shifted.to_ascii_lowercase() == lower_c
    }).map(|&(keycode, _, _)| keycode)
}

pub fn key_to_char(key: KeyCode, shift: bool) -> Option<char> {
    KEY_MAP.iter().find(|&&(keycode, _, _)| keycode == key)
        .map(|&(_, normal, shifted)| if shift { shifted } else { normal })
}

#[derive(Clone)]
pub enum InputEvent {
    Expansion(usize, Trigger),
    AiFix(String),
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
        let name_lower = name.to_lowercase();
        let is_remapper = name_lower.contains("keyd") || name_lower.contains("kmonad") || name_lower.contains("kanata");

        if is_remapper {
            virtual_kbd = Some((path, device));
        } else if !name_lower.contains("virtual") {
            keyboards.push((path, device));
        }
    }

    if let Some(vkbd) = virtual_kbd {
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
    ctrl: bool,
    alt: bool,
    meta: bool,
    ai_matches: Vec<(Hotkey, String)>,
}

impl TextExpander {
    pub fn new(
        triggers: HashMap<String, Trigger>,
        ai_config: Option<&crate::config::AiConfig>,
        initial_capslock: bool,
    ) -> Self {
        let max_len = triggers.keys().map(|k| k.len()).max().unwrap_or(64) + 1;
        let mut sorted_triggers: Vec<(String, Trigger)> = triggers.into_iter().collect();
        sorted_triggers.sort_by(|a, b| {
            b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0))
        });

        let mut ai_matches = Vec::new();
        if let Some(ai) = ai_config {
            for m in &ai.matches {
                if let Some(hk) = Hotkey::parse(&m.hotkey) {
                    ai_matches.push((hk, m.prompt.clone()));
                } else {
                    eprintln!("\x1b[33m⚠️  [config] Warning:\x1b[0m Failed to parse hotkey: {}", m.hotkey);
                }
            }
        }

        Self {
            sorted_triggers,
            buffer: String::with_capacity(max_len),
            max_len,
            shift: false,
            capslock: initial_capslock,
            ctrl: false,
            alt: false,
            meta: false,
            ai_matches,
        }
    }

    fn update_modifiers_down(&self) {
        let mut mask = 0;
        if self.ctrl { mask |= 1; }
        if self.alt { mask |= 2; }
        if self.shift { mask |= 4; }
        if self.meta { mask |= 8; }
        MODIFIERS_DOWN.store(mask, Ordering::SeqCst);
    }

    pub fn process(&mut self, key: KeyCode, pressed: bool) -> Option<InputEvent> {
        if key == KeyCode::KEY_LEFTSHIFT || key == KeyCode::KEY_RIGHTSHIFT {
            self.shift = pressed;
            self.update_modifiers_down();
            return None;
        }

        if key == KeyCode::KEY_LEFTCTRL || key == KeyCode::KEY_RIGHTCTRL {
            self.ctrl = pressed;
            self.update_modifiers_down();
            return None;
        }

        if key == KeyCode::KEY_LEFTALT || key == KeyCode::KEY_RIGHTALT {
            self.alt = pressed;
            self.update_modifiers_down();
            return None;
        }

        if key == KeyCode::KEY_LEFTMETA || key == KeyCode::KEY_RIGHTMETA {
            self.meta = pressed;
            self.update_modifiers_down();
            return None;
        }

        if key == KeyCode::KEY_CAPSLOCK && pressed {
            self.capslock = !self.capslock;
            return None;
        }

        if pressed {
            for (hk, prompt) in &self.ai_matches {
                if self.ctrl == hk.ctrl && self.alt == hk.alt && self.shift == hk.shift && self.meta == hk.meta && key == hk.key {
                    self.buffer.clear();
                    return Some(InputEvent::AiFix(prompt.clone()));
                }
            }
        }

        if !pressed { return None }

        if self.ctrl || self.alt || self.meta {
            self.buffer.clear();
            return None;
        }

        match key {
            KeyCode::KEY_ENTER | KeyCode::KEY_TAB | KeyCode::KEY_ESC |
            KeyCode::KEY_LEFT | KeyCode::KEY_RIGHT | KeyCode::KEY_UP | KeyCode::KEY_DOWN |
            KeyCode::KEY_HOME | KeyCode::KEY_END | KeyCode::KEY_PAGEUP | KeyCode::KEY_PAGEDOWN |
            KeyCode::KEY_DELETE => {
                self.buffer.clear();
                return None;
            }
            KeyCode::KEY_BACKSPACE => {
                self.buffer.pop();
                return None;
            }
            _ => {}
        }

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

                    let result = InputEvent::Expansion(trig.len(), data.clone());
                    self.buffer.clear();
                    return Some(result);
                }
            }
        }
        None
    }
}

