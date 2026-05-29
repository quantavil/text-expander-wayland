use evdev::{Device, EventType, KeyCode};
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    fs,
    os::unix::io::AsRawFd,
    path::PathBuf,
    process,
    thread,
    time::Duration,
};

// Espanso-compatible config format
#[derive(Debug, Deserialize)]
struct EspansoConfig {
    #[serde(default)]
    matches: Vec<Match>,
    #[serde(default)]
    global_vars: Vec<Var>,
}

#[derive(Debug, Deserialize)]
struct Match {
    trigger: Option<String>,
    #[serde(default)]
    triggers: Vec<String>,
    replace: Option<String>,
    #[serde(default)]
    vars: Vec<Var>,
}

#[derive(Debug, Clone, Deserialize)]
struct Var {
    name: String,
    #[serde(rename = "type")]
    var_type: String,
    #[serde(default)]
    params: VarParams,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct VarParams {
    format: Option<String>,
    cmd: Option<String>,
    echo: Option<String>,
}

#[derive(Clone)]
struct Trigger {
    replace: String,
    vars: Vec<Var>,
}

impl Trigger {
    fn expand(&self) -> String {
        let mut result = self.replace.clone();

        for var in &self.vars {
            let value = match var.var_type.as_str() {
                "date" => {
                    let fmt = var.params.format.as_deref().unwrap_or("%Y-%m-%d");
                    run_command("date", &[&format!("+{}", fmt)])
                }
                "shell" => {
                    if let Some(cmd) = &var.params.cmd {
                        run_command("sh", &["-c", cmd])
                    } else {
                        String::new()
                    }
                }
                "clipboard" => run_command("wl-paste", &["-n"]),
                "echo" => var.params.echo.as_ref()
                    .or(var.params.format.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                _ => format!("{{{{{}}}}}", var.name),
            };
            result = result.replace(&format!("{{{{{}}}}}", var.name), &value);
        }
        result
    }
}

fn run_command(cmd_name: &str, args: &[&str]) -> String {
    if let Ok(sudo_user) = env::var("SUDO_USER") {
        let mut cmd = process::Command::new("sudo");
        cmd.arg("-u").arg(&sudo_user).arg("env");
        for (k, v) in get_wayland_env() {
            cmd.arg(format!("{}={}", k, v));
        }
        cmd.arg(cmd_name).args(args);
        cmd.output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        process::Command::new(cmd_name)
            .args(args)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }
}

fn key_to_char(key: KeyCode, shift: bool) -> Option<char> {
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

fn load_yaml_recursive(dir: &PathBuf, triggers: &mut HashMap<String, Trigger>, global_vars: &mut Vec<Var>) {
    let Ok(entries) = fs::read_dir(dir) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            load_yaml_recursive(&path, triggers, global_vars);
        } else if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
            let Ok(content) = fs::read_to_string(&path) else { continue };
            match serde_saphyr::from_str::<EspansoConfig>(&content) {
                Ok(config) => {
                    global_vars.extend(config.global_vars);
                    let mut count = 0;
                    for m in config.matches {
                        let Some(replace) = m.replace else { continue };

                        // Collect all triggers: singular `trigger` and plural `triggers`
                        let mut all_triggers = Vec::new();
                        if let Some(t) = m.trigger {
                            all_triggers.push(t);
                        }
                        all_triggers.extend(m.triggers);

                        for trig in all_triggers {
                            triggers.insert(trig, Trigger {
                                replace: replace.clone(),
                                vars: m.vars.clone(),
                            });
                            count += 1;
                        }
                    }
                    if count > 0 {
                        eprintln!("Loaded {} triggers from {:?}", count, path);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: failed to parse {:?}: {}", path, e);
                }
            }
        }
    }
}

fn load_configs() -> HashMap<String, Trigger> {
    let mut triggers = HashMap::new();
    let mut global_vars = Vec::new();
    let config_dir = get_config_path();

    if config_dir.exists() {
        load_yaml_recursive(&config_dir, &mut triggers, &mut global_vars);
    } else {
        eprintln!("Config directory not found: {:?}", config_dir);
    }

    // Append global_vars to each trigger's vars, skipping any that the trigger
    // already defines locally (so local vars can override globals)
    if !global_vars.is_empty() {
        for trigger in triggers.values_mut() {
            let local_names: std::collections::HashSet<String> =
                trigger.vars.iter().map(|v| v.name.clone()).collect();
            for gv in &global_vars {
                if !local_names.contains(gv.name.as_str()) {
                    trigger.vars.push(gv.clone());
                }
            }
        }
    }

    triggers
}

fn get_config_path() -> PathBuf {
    let home = env::var("SUDO_USER")
        .ok()
        .and_then(|user| {
            fs::read_to_string("/etc/passwd").ok().and_then(|passwd| {
                passwd.lines()
                    .find(|l| l.starts_with(&format!("{}:", user)))
                    .and_then(|l| l.split(':').nth(5))
                    .map(String::from)
            })
        })
        .or_else(|| env::var("HOME").ok())
        .unwrap_or_else(|| "/tmp".into());

    PathBuf::from(home).join(".config/text_expander")
}

fn find_keyboards() -> Vec<Device> {
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
        eprintln!("Found keyboard: {:?} - {}", path, name);

        let name_lower = name.to_lowercase();
        let is_remapper = name_lower.contains("keyd") || name_lower.contains("kmonad") || name_lower.contains("kanata");

        if is_remapper {
            virtual_kbd = Some(device);
        } else if !name_lower.contains("virtual") {
            keyboards.push(device);
        }
    }

    if let Some(vkbd) = virtual_kbd {
        eprintln!("Using virtual keyboard only (keyd/kmonad/kanata detected)");
        vec![vkbd]
    } else {
        keyboards
    }
}

fn get_wayland_env() -> Vec<(String, String)> {
    let mut env_vars = Vec::new();
    let real_uid = env::var("SUDO_UID").unwrap_or_default();

    let xdg_runtime = if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        xdg
    } else if !real_uid.is_empty() {
        format!("/run/user/{}", real_uid)
    } else {
        String::new()
    };

    if !xdg_runtime.is_empty() {
        env_vars.push(("XDG_RUNTIME_DIR".into(), xdg_runtime.clone()));
    }

    let mut wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-1".into());
    if !xdg_runtime.is_empty() {
        if let Ok(entries) = fs::read_dir(&xdg_runtime) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("wayland-") && !name.ends_with(".lock") {
                    wayland_display = name;
                    break;
                }
            }
        }
    }

    env_vars.push(("WAYLAND_DISPLAY".into(), wayland_display));

    if let Ok(user) = env::var("SUDO_USER") {
        env_vars.push(("USER".into(), user));
    }
    env_vars
}

fn get_ydotool_socket_path() -> Option<PathBuf> {
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

fn run_wtype(args: &[&str]) {
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

fn type_expansion(backspaces: usize, text: &str) {
    if let Some(socket_path) = get_ydotool_socket_path() {
        if backspaces > 0 {
            let mut key_args = Vec::new();
            for _ in 0..backspaces {
                key_args.push("14:1");
                key_args.push("14:0");
            }
            let refs: Vec<&str> = key_args.iter().map(|s| *s).collect();
            let mut cmd = process::Command::new("ydotool");
            cmd.env("YDOTOOL_SOCKET", &socket_path);
            cmd.arg("key").arg("-d").arg("0").args(&refs);
            let _ = cmd.status();
        }
        let mut cmd = process::Command::new("ydotool");
        cmd.env("YDOTOOL_SOCKET", &socket_path);
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

struct TextExpander {
    triggers: HashMap<String, Trigger>,
    buffer: String,
    max_len: usize,
    shift: bool,
    capslock: bool,
}

impl TextExpander {
    fn new(triggers: HashMap<String, Trigger>) -> Self {
        let max_len = triggers.keys().map(|k| k.len()).max().unwrap_or(64);
        Self { triggers, buffer: String::with_capacity(max_len + 1), max_len, shift: false, capslock: false }
    }

    fn process(&mut self, key: KeyCode, pressed: bool) -> Option<(usize, String)> {
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

        let effective_shift = self.shift ^ self.capslock;
        if let Some(c) = key_to_char(key, effective_shift) {
            self.buffer.push(c);
            if self.buffer.len() > self.max_len {
                self.buffer.drain(..self.buffer.len() - self.max_len);
            }

            for (trig, data) in &self.triggers {
                if self.buffer.ends_with(trig) {
                    let result = (trig.len(), data.expand());
                    self.buffer.clear();
                    return Some(result);
                }
            }
        }
        None
    }
}

fn daemonize() {
    // Fork and exit parent
    match unsafe { libc::fork() } {
        -1 => { eprintln!("Fork failed"); process::exit(1); }
        0 => {} // Child continues
        _ => process::exit(0), // Parent exits
    }

    // Create new session
    if unsafe { libc::setsid() } == -1 {
        eprintln!("setsid failed");
        process::exit(1);
    }

    // Redirect stdio to /dev/null
    let devnull = fs::OpenOptions::new()
        .read(true).write(true).open("/dev/null").unwrap();

    unsafe {
        libc::dup2(devnull.as_raw_fd(), 0);
        libc::dup2(devnull.as_raw_fd(), 1);
        libc::dup2(devnull.as_raw_fd(), 2);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let daemon_mode = args.iter().any(|a| a == "-d" || a == "--daemon");

    eprintln!("text_expander - lightweight espanso replacement for Wayland");

    let triggers = load_configs();
    if triggers.is_empty() {
        eprintln!("No triggers loaded. Create config in ~/.config/text_expander/");
        process::exit(1);
    }
    eprintln!("Loaded {} triggers", triggers.len());

    let mut keyboards = find_keyboards();
    if keyboards.is_empty() {
        eprintln!("No keyboards found. Need read access to /dev/input/*");
        process::exit(1);
    }

    if daemon_mode {
        eprintln!("Daemonizing...");
        daemonize();
    } else {
        eprintln!("Ready! (use -d/--daemon to run in background)");
    }

    let mut expander = TextExpander::new(triggers);

    loop {
        let raw_fds: Vec<i32> = keyboards.iter().map(|k| k.as_raw_fd()).collect();
        let mut pollfds: Vec<libc::pollfd> = raw_fds.iter()
            .map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            .collect();

        if unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, -1) } < 0 {
            continue;
        }

        let mut i = pollfds.len();
        while i > 0 {
            i -= 1;
            if pollfds[i].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                eprintln!("Keyboard disconnected (fd {}), removing", raw_fds[i]);
                keyboards.remove(i);
            }
        }
        if keyboards.is_empty() {
            eprintln!("All keyboards disconnected, exiting");
            process::exit(0);
        }

        let ready: Vec<usize> = pollfds.iter().enumerate()
            .filter(|(_, p)| p.revents & libc::POLLIN != 0)
            .map(|(i, _)| i).collect();


        for &i in &ready {
            if i >= keyboards.len() { continue }
            if let Ok(events) = keyboards[i].fetch_events() {
                for ev in events {
                    if ev.event_type() == EventType::KEY {
                        if let Some((n, text)) = expander.process(KeyCode::new(ev.code()), ev.value() == 1) {
                            thread::sleep(Duration::from_millis(10));
                            type_expansion(n, &text);
                        }
                    }
                }
            }
        }
    }
}
