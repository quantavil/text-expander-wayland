use evdev::KeyCode;
use std::{
    env,
    path::PathBuf,
    process,
    thread,
    time::Duration,
};
use crate::config::{run_command, get_wayland_env};
use crate::input::key_to_char;

pub fn copy_to_clipboard(text: &str) {
    let mut cmd = if let Ok(sudo_user) = env::var("SUDO_USER") {
        let mut c = process::Command::new("sudo");
        c.arg("-u").arg(&sudo_user).arg("env");
        for (k, v) in get_wayland_env() {
            c.arg(format!("{}={}", k, v));
        }
        c.arg("wl-copy");
        c
    } else {
        process::Command::new("wl-copy")
    };

    cmd.stdin(process::Stdio::piped());
    if let Ok(mut child) = cmd.spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

pub fn run_wtype(args: &[&str]) {
    if let Ok(sudo_user) = env::var("SUDO_USER") {
        let mut cmd = process::Command::new("sudo");
        cmd.arg("-u").arg(&sudo_user).arg("env");
        for (k, v) in get_wayland_env() {
            cmd.arg(format!("{}={}", k, v));
        }
        cmd.arg("wtype").args(args);
        let _ = cmd.status();
    } else {
        let _ = process::Command::new("wtype").args(args).status();
    }
}

pub fn get_ydotool_socket_path() -> Option<PathBuf> {
    let real_uid = env::var("SUDO_UID").unwrap_or_default();
    let xdg_runtime = if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        xdg
    } else if !real_uid.is_empty() {
        format!("/run/user/{}", real_uid)
    } else {
        String::new()
    };

    if !xdg_runtime.is_empty() {
        let path = PathBuf::from(&xdg_runtime).join(".ydotool_socket");
        if path.exists() {
            return Some(path);
        }
    }

    // Direct fallback check for uid 1000
    let path = PathBuf::from("/run/user/1000/.ydotool_socket");
    if path.exists() {
        return Some(path);
    }

    None
}

pub fn has_key_conflict(text: &str, last_key: KeyCode) -> bool {
    let Some(tc_lower) = key_to_char(last_key, false) else { return false };
    let tc_upper = key_to_char(last_key, true).unwrap_or(tc_lower);

    // Check if the trigger key character appears (case-insensitively) in the first 15 characters
    let prefix_len = std::cmp::min(text.len(), 15);
    let prefix = &text[..prefix_len];
    prefix.contains(tc_lower) || prefix.contains(tc_upper)
}

pub fn type_expansion(backspaces: usize, text: &str, last_key: KeyCode) {
    let socket_path = get_ydotool_socket_path();

    // Check if we should paste using the clipboard to avoid character-by-character typing bugs.
    let use_paste = text.contains('\n')
        || text.contains('\r')
        || text.contains('\t')
        || text.len() > 25
        || has_key_conflict(text, last_key);

    if use_paste {
        // Only run wl-paste if we actually need to paste, saving process spawn overhead.
        let saved_clipboard = run_command("wl-paste", &["-n"]);
        let is_clipboard_identical = text == saved_clipboard;

        // 1. Copy the text to clipboard if it's not already there
        if !is_clipboard_identical {
            copy_to_clipboard(text);
        }

        // 2. Delete the trigger characters (backspaces)
        if backspaces > 0 {
            if let Some(ref socket) = socket_path {
                let mut key_args = Vec::new();
                for _ in 0..backspaces {
                    key_args.push("14:1");
                    key_args.push("14:0");
                }
                let refs: Vec<&str> = key_args.iter().map(|s| *s).collect();
                let mut cmd = process::Command::new("ydotool");
                cmd.env("YDOTOOL_SOCKET", socket);
                cmd.arg("key").arg("-d").arg("0").args(&refs);
                let _ = cmd.status();
            } else {
                let mut args: Vec<String> = Vec::new();
                for _ in 0..backspaces {
                    args.push("-k".into());
                    args.push("BackSpace".into());
                }
                let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                run_wtype(&refs);
            }
            // Give a tiny moment for backspaces to register in the target application
            thread::sleep(Duration::from_millis(30));
        }

        // 3. Simulate Ctrl+V to paste the text
        if let Some(ref socket) = socket_path {
            let mut cmd = process::Command::new("ydotool");
            cmd.env("YDOTOOL_SOCKET", socket);
            cmd.arg("key").arg("-d").arg("0").args(&["29:1", "47:1", "47:0", "29:0"]);
            let _ = cmd.status();
        } else {
            run_wtype(&["-M", "ctrl", "-k", "v", "-m", "ctrl"]);
        }

        // 4. Restore original clipboard content in a background thread if we modified it
        if !is_clipboard_identical {
            // Restore clipboard in a background thread after a delay to allow the application
            // to complete the paste request, without blocking the main event loop.
            let saved = saved_clipboard.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(800));
                copy_to_clipboard(&saved);
            });
        }
    } else {
        // Fallback: character-by-character simulated typing
        if let Some(ref socket) = socket_path {
            if backspaces > 0 {
                let mut key_args = Vec::new();
                for _ in 0..backspaces {
                    key_args.push("14:1");
                    key_args.push("14:0");
                }
                let refs: Vec<&str> = key_args.iter().map(|s| *s).collect();
                let mut cmd = process::Command::new("ydotool");
                cmd.env("YDOTOOL_SOCKET", socket);
                cmd.arg("key").arg("-d").arg("0").args(&refs);
                let _ = cmd.status();
            }
            let mut cmd = process::Command::new("ydotool");
            cmd.env("YDOTOOL_SOCKET", socket);
            cmd.arg("type").arg("-d").arg("1").arg("-H").arg("1").arg(text);
            let _ = cmd.status();
        } else {
            let mut args: Vec<String> = Vec::new();
            for _ in 0..backspaces {
                args.push("-k".into());
                args.push("BackSpace".into());
            }
            args.push("--".into());
            args.push(text.into());

            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_wtype(&refs);
        }
    }
}
