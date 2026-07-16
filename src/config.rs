use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    fs,
    path::PathBuf,
    process,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AiConfig {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub matches: Vec<AiMatchConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AiMatchConfig {
    pub hotkey: String,
    pub prompt: String,
}

// Espanso-compatible config format
#[derive(Debug, Deserialize)]
pub struct EspansoConfig {
    #[serde(default)]
    pub matches: Vec<Match>,
    #[serde(default)]
    pub global_vars: Vec<Var>,
    pub ai: Option<AiConfig>,
}

#[derive(Clone)]
pub struct Config {
    pub triggers: std::collections::HashMap<String, Trigger>,
    pub ai: Option<std::sync::Arc<AiConfig>>,
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
    pub vars: std::sync::Arc<Vec<Var>>,
}

impl Trigger {
    pub fn expand(&self) -> String {
        let mut result = self.replace.clone();

        for var in self.vars.iter() {
            let placeholder = format!("{{{{{}}}}}", var.name);
            if !result.contains(&placeholder) {
                continue;
            }
            let value = match var.var_type.as_str() {
                "date" => {
                    let fmt = var.params.format.as_deref().unwrap_or("%Y-%m-%d");
                    chrono::Local::now().format(fmt).to_string()
                }
                "shell" => {
                    if let Some(cmd) = &var.params.cmd {
                        run_command("sh", &["-c", cmd]).trim().to_string()
                    } else {
                        String::new()
                    }
                }
                "clipboard" => run_command("wl-paste", &["-n"]),
                "echo" => var.params.echo.as_ref()
                    .or(var.params.format.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                _ => placeholder.clone(),
            };
            result = result.replace(&placeholder, &value);
        }
        result
    }
}

pub fn get_sudo_user() -> Option<&'static String> {
    static CACHE: OnceLock<Option<String>> = OnceLock::new();
    CACHE.get_or_init(|| env::var("SUDO_USER").ok()).as_ref()
}

pub fn get_wayland_env() -> &'static [(String, String)] {
    static CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    CACHE.get_or_init(|| {
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

        if let Some(user) = get_sudo_user() {
            env_vars.push(("USER".into(), user.clone()));
        }
        env_vars
    })
}

pub fn user_cmd(prog: &str) -> process::Command {
    if let Some(sudo_user) = get_sudo_user() {
        let mut cmd = process::Command::new("sudo");
        cmd.arg("-u").arg(sudo_user).arg("env");
        for (k, v) in get_wayland_env() {
            cmd.arg(format!("{}={}", k, v));
        }
        cmd.arg(prog);
        cmd
    } else {
        process::Command::new(prog)
    }
}

pub fn run_command(cmd_name: &str, args: &[&str]) -> String {
    let mut cmd = user_cmd(cmd_name);
    cmd.args(args);
    cmd.output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
}

pub fn load_yaml_recursive(
    dir: &PathBuf,
    triggers: &mut HashMap<String, Trigger>,
    ai_config: &mut Option<AiConfig>,
) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            load_yaml_recursive(&path, triggers, ai_config);
        } else if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            let Ok(content) = fs::read_to_string(&path) else { continue };
            match serde_norway::from_str::<EspansoConfig>(&content) {
                Ok(config) => {
                    if let Some(ai) = config.ai {
                        if let Some(ref mut existing) = ai_config {
                            if existing.api_key.is_none() {
                                existing.api_key = ai.api_key;
                            }
                            if existing.endpoint.is_none() {
                                existing.endpoint = ai.endpoint;
                            }
                            if existing.model.is_none() {
                                existing.model = ai.model;
                            }
                            existing.matches.extend(ai.matches);
                        } else {
                            *ai_config = Some(ai);
                        }
                    }

                    let mut count = 0;
                    for m in config.matches {
                        let Some(replace) = m.replace else { continue };

                        let mut all_triggers = Vec::new();
                        if let Some(t) = m.trigger {
                            all_triggers.push(t);
                        }
                        all_triggers.extend(m.triggers);

                        let mut local_vars = m.vars.clone();
                        let local_names: std::collections::HashSet<String> =
                            local_vars.iter().map(|v| v.name.clone()).collect();
                        for gv in &config.global_vars {
                            if !local_names.contains(&gv.name) {
                                local_vars.push(gv.clone());
                            }
                        }
                        let local_vars_arc = Arc::new(local_vars);

                        for trig in all_triggers {
                            if triggers.insert(trig.clone(), Trigger {
                                replace: replace.clone(),
                                vars: local_vars_arc.clone(),
                            }).is_some() {
                                eprintln!("\x1b[33m⚠️  [config] Warning:\x1b[0m Duplicate trigger key '{}' detected in {:?}, overwriting", trig, path);
                            }
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

pub fn load_configs() -> Config {
    let mut triggers = HashMap::new();
    let mut ai_config = None;
    let config_dir = get_config_path();

    if config_dir.exists() {
        load_yaml_recursive(&config_dir, &mut triggers, &mut ai_config);
    } else {
        eprintln!("Config directory not found: {:?}", config_dir);
    }

    Config {
        triggers,
        ai: ai_config.map(Arc::new),
    }
}

pub fn get_config_path() -> PathBuf {
    let home = get_sudo_user()
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
