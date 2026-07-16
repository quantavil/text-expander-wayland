use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    fs,
    path::PathBuf,
    process,
};

// Espanso-compatible config format
#[derive(Debug, Deserialize)]
pub struct EspansoConfig {
    #[serde(default)]
    pub matches: Vec<Match>,
    #[serde(default)]
    pub global_vars: Vec<Var>,
}

#[derive(Debug, Deserialize)]
pub struct Match {
    pub trigger: Option<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    pub replace: Option<String>,
    #[serde(default)]
    pub vars: Vec<Var>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Var {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    #[serde(default)]
    pub params: VarParams,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VarParams {
    pub format: Option<String>,
    pub cmd: Option<String>,
    pub echo: Option<String>,
}

#[derive(Clone)]
pub struct Trigger {
    pub replace: String,
    pub vars: Vec<Var>,
}

impl Trigger {
    pub fn expand(&self) -> String {
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

pub fn run_command(cmd_name: &str, args: &[&str]) -> String {
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

pub fn get_wayland_env() -> Vec<(String, String)> {
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

pub fn load_yaml_recursive(dir: &PathBuf, triggers: &mut HashMap<String, Trigger>, global_vars: &mut Vec<Var>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
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
                        eprintln!("\x1b[36m✨ [config]\x1b[0m Loaded {} triggers from {:?}", count, path);
                    }
                }
                Err(e) => {
                    eprintln!("\x1b[33m⚠️  [config] Warning:\x1b[0m failed to parse {:?}: {}", path, e);
                }
            }
        }
    }
}

pub fn load_configs() -> HashMap<String, Trigger> {
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

pub fn get_config_path() -> PathBuf {
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
