use evdev::KeyCode;
use std::{
    env,
    path::PathBuf,
    process,
    thread,
    time::Duration,
    sync::OnceLock,
};
use crate::config::{run_command, user_cmd};
use crate::input::key_to_char;

const CLIPBOARD_PASTE_THRESHOLD: usize = 25;
const KEY_CONFLICT_CHECK_LEN: usize = 25;
const TYPING_DELAY_MS: u64 = 30;

static ACTIVE_CLIPBOARD_EXPANSIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ORIGINAL_CLIPBOARD: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

const KEYCODE_LEFTCTRL: u16 = 29;
const KEYCODE_C: u16 = 46;
const KEYCODE_V: u16 = 47;
const KEYCODE_BACKSPACE: u16 = 14;
const KEYCODE_LEFT: u16 = 105;

pub fn copy_to_clipboard(text: &str) {
    let mut cmd = user_cmd("wl-copy");
    cmd.stdin(process::Stdio::piped());
    if let Ok(mut child) = cmd.spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

pub fn clear_clipboard() {
    let mut cmd = user_cmd("wl-copy");
    cmd.arg("--clear");
    let _ = cmd.status();
}

pub fn run_wtype(args: &[&str]) {
    let mut cmd = user_cmd("wtype");
    cmd.args(args);
    let _ = cmd.status();
}

pub fn get_ydotool_socket_path() -> Option<&'static PathBuf> {
    static CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();
    CACHE.get_or_init(|| {
        if let Ok(socket_env) = env::var("YDOTOOL_SOCKET") {
            let path = PathBuf::from(socket_env);
            if path.exists() {
                return Some(path);
            }
        }

        let real_uid = env::var("SUDO_UID").unwrap_or_default();
        let uid = if !real_uid.is_empty() {
            real_uid
        } else {
            unsafe { libc::getuid() }.to_string()
        };

        let xdg_runtime = if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
            xdg
        } else {
            format!("/run/user/{}", uid)
        };

        let path = PathBuf::from(&xdg_runtime).join(".ydotool_socket");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }).as_ref()
}

fn ydotool_or_wtype(ydotool_args: Vec<String>, wtype_args: Vec<String>) {
    if let Some(socket) = get_ydotool_socket_path() {
        let mut cmd = process::Command::new("ydotool");
        cmd.env("YDOTOOL_SOCKET", socket);
        cmd.args(ydotool_args);
        let _ = cmd.status();
    } else {
        let refs: Vec<&str> = wtype_args.iter().map(|s| s.as_str()).collect();
        run_wtype(&refs);
    }
}

pub fn simulate_copy() {
    ydotool_or_wtype(
        vec!["key".into(), "-d".into(), "0".into(), format!("{}:1", KEYCODE_LEFTCTRL), format!("{}:1", KEYCODE_C), format!("{}:0", KEYCODE_C), format!("{}:0", KEYCODE_LEFTCTRL)],
        vec!["-M".into(), "ctrl".into(), "-k".into(), "c".into(), "-m".into(), "ctrl".into()],
    );
}

pub fn simulate_paste() {
    ydotool_or_wtype(
        vec!["key".into(), "-d".into(), "0".into(), format!("{}:1", KEYCODE_LEFTCTRL), format!("{}:1", KEYCODE_V), format!("{}:0", KEYCODE_V), format!("{}:0", KEYCODE_LEFTCTRL)],
        vec!["-M".into(), "ctrl".into(), "-k".into(), "v".into(), "-m".into(), "ctrl".into()],
    );
}

pub fn simulate_backspaces(backspaces: usize) {
    if backspaces == 0 { return; }
    let mut ydo = vec!["key".into(), "-d".into(), "0".into()];
    let mut wt = Vec::new();
    for _ in 0..backspaces {
        ydo.push(format!("{}:1", KEYCODE_BACKSPACE));
        ydo.push(format!("{}:0", KEYCODE_BACKSPACE));
        wt.push("-k".into());
        wt.push("BackSpace".into());
    }
    ydotool_or_wtype(ydo, wt);
}

pub fn simulate_cursor_move(moves: usize) {
    if moves == 0 { return; }
    let mut ydo = vec!["key".into(), "-d".into(), "0".into()];
    let mut wt = Vec::new();
    for _ in 0..moves {
        ydo.push(format!("{}:1", KEYCODE_LEFT));
        ydo.push(format!("{}:0", KEYCODE_LEFT));
        wt.push("-k".into());
        wt.push("Left".into());
    }
    ydotool_or_wtype(ydo, wt);
}

pub fn simulate_type_fallback(text: &str) {
    ydotool_or_wtype(
        vec!["type".into(), "-d".into(), "1".into(), "-H".into(), "1".into(), text.to_string()],
        vec!["--".into(), text.to_string()],
    );
}

pub fn has_key_conflict(text: &str, last_key: Option<KeyCode>) -> bool {
    let Some(key) = last_key else { return false };
    let Some(tc_lower) = key_to_char(key, false) else { return false };
    let tc_upper = key_to_char(key, true).unwrap_or(tc_lower);

    text.chars().take(KEY_CONFLICT_CHECK_LEN).any(|c| c == tc_lower || c == tc_upper)
}

pub fn type_expansion(backspaces: usize, text: &str, last_key: Option<KeyCode>, force_paste: bool) {
    for _ in 0..50 {
        if crate::input::MODIFIERS_DOWN.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let (actual_text, cursor_moves) = if let Some(pos) = text.find("$|$") {
        let prefix = &text[..pos];
        let suffix = &text[pos + 3..];
        (format!("{}{}", prefix, suffix), suffix.chars().count())
    } else {
        (text.to_string(), 0)
    };

    let use_paste = force_paste
        || actual_text.contains('\n')
        || actual_text.contains('\r')
        || actual_text.contains('\t')
        || actual_text.len() > CLIPBOARD_PASTE_THRESHOLD
        || has_key_conflict(&actual_text, last_key);

    if use_paste {
        let current_clipboard = run_command("wl-paste", &["-n"]);

        if ACTIVE_CLIPBOARD_EXPANSIONS.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            let mut guard = ORIGINAL_CLIPBOARD.lock().unwrap();
            *guard = current_clipboard.clone();
        }

        ACTIVE_CLIPBOARD_EXPANSIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let is_clipboard_identical = actual_text == current_clipboard;

        if !is_clipboard_identical {
            copy_to_clipboard(&actual_text);
        }

        simulate_backspaces(backspaces);
        if backspaces > 0 {
            thread::sleep(Duration::from_millis(TYPING_DELAY_MS));
        }

        simulate_paste();

        if cursor_moves > 0 {
            thread::sleep(Duration::from_millis(TYPING_DELAY_MS));
            simulate_cursor_move(cursor_moves);
        }

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(800));
            if ACTIVE_CLIPBOARD_EXPANSIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                let guard = ORIGINAL_CLIPBOARD.lock().unwrap();
                copy_to_clipboard(&guard);
            }
        });
    } else {
        simulate_backspaces(backspaces);
        simulate_type_fallback(&actual_text);
        simulate_cursor_move(cursor_moves);
    }
}
