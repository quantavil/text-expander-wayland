use text_expander::{
    ai,
    config::load_configs,
    inject::type_expansion,
    input::{find_keyboards, TextExpander, InputEvent},
};

use evdev::{EventType, KeyCode};
use std::{
    env,
    os::unix::io::AsRawFd,
    process,
    thread,
    sync::atomic::{AtomicBool, Ordering},
};

static AI_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: text_expander [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -t, --list-triggers    List all loaded triggers and exit");
        println!("  -v, --version          Show version information and exit");
        println!("  -h, --help             Show this help menu and exit");
        process::exit(0);
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("text_expander {}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }



    let config = load_configs();

    if args.iter().any(|a| a == "-t" || a == "--list-triggers") {
        println!("Loaded triggers ({}):", config.triggers.len());
        let mut sorted_keys: Vec<_> = config.triggers.keys().collect();
        sorted_keys.sort();
        for trigger in sorted_keys {
            println!("  {}", trigger);
        }
        process::exit(0);
    }

    eprintln!("\x1b[1m🚀 [text_expander]\x1b[0m lightweight espanso replacement for Wayland");

    if config.triggers.is_empty() {
        eprintln!("\x1b[31m❌ [config] Error:\x1b[0m No triggers loaded. Create config in ~/.config/text_expander/");
        process::exit(1);
    }
    eprintln!("\x1b[32m📦 [config]\x1b[0m Loaded {} triggers total", config.triggers.len());

    let mut keyboards = find_keyboards();
    if keyboards.is_empty() {
        eprintln!("\x1b[31m❌ [input] Error:\x1b[0m No keyboards found. Need read access to /dev/input/*");
        process::exit(1);
    }
    for (path, device) in &keyboards {
        let name = device.name().unwrap_or("unknown");
        eprintln!("\x1b[34m⌨️  [input]\x1b[0m Found keyboard: {:?} - {}", path, name);
    }

    let mut initial_capslock = false;
    for (_, device) in &keyboards {
        if let Ok(led_state) = device.get_led_state() {
            if led_state.contains(evdev::LedCode::LED_CAPSL) {
                initial_capslock = true;
                break;
            }
        }
    }

    eprintln!("\x1b[32m🟢 [daemon]\x1b[0m Ready!");

    let mut expander = TextExpander::new(config.triggers, config.ai.as_deref(), initial_capslock);

    let (tx, rx) = std::sync::mpsc::channel::<(usize, String, Option<KeyCode>)>();
    thread::spawn(move || {
        while let Ok((n, text, last_key)) = rx.recv() {
            type_expansion(n, &text, last_key, false);
        }
    });

    let mut last_scan = std::time::Instant::now();

    loop {
        let now = std::time::Instant::now();
        if now.duration_since(last_scan) >= std::time::Duration::from_secs(5) {
            last_scan = now;
            let scanned = find_keyboards();
            let scanned_paths: std::collections::HashSet<_> = scanned.iter().map(|(p, _)| p.clone()).collect();
            
            let mut i = keyboards.len();
            while i > 0 {
                i -= 1;
                if !scanned_paths.contains(&keyboards[i].0) {
                    eprintln!("\x1b[33m⚠️  [input]\x1b[0m Keyboard removed: {:?}", keyboards[i].0);
                    keyboards.remove(i);
                }
            }
            
            for (path, device) in scanned {
                if !keyboards.iter().any(|(p, _)| *p == path) {
                    eprintln!("\x1b[36m🔌 [input]\x1b[0m New keyboard hotplugged: {:?}", path);
                    keyboards.push((path, device));
                }
            }
            if keyboards.is_empty() {
                eprintln!("\x1b[31m❌ [input]\x1b[0m All keyboards disconnected, exiting");
                process::exit(1);
            }
        }

        let mut pollfds: Vec<libc::pollfd> = keyboards.iter()
            .map(|(_, k)| libc::pollfd { fd: k.as_raw_fd(), events: libc::POLLIN, revents: 0 })
            .collect();

        let poll_res = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 1000) };
        if poll_res < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            eprintln!("\x1b[31m❌ [input] poll error:\x1b[0m {}", err);
            process::exit(1);
        }

        if poll_res == 0 {
            continue;
        }

        let ready: Vec<usize> = pollfds.iter().enumerate()
            .filter(|(_, p)| p.revents & libc::POLLIN != 0)
            .map(|(i, _)| i).collect();

        for &i in &ready {
            if let Ok(events) = keyboards[i].1.fetch_events() {
                for ev in events {
                    if ev.event_type() == EventType::KEY {
                        let val = ev.value();
                        if val == 0 || val == 1 {
                            if let Some(event) = expander.process(KeyCode::new(ev.code()), val == 1) {
                                match event {
                                    InputEvent::Expansion(n, trigger) => {
                                        let tx_typist = tx.clone();
                                        let last_key = Some(KeyCode::new(ev.code()));
                                        thread::spawn(move || {
                                            let text = trigger.expand();
                                            if let Err(e) = tx_typist.send((n, text, last_key)) {
                                                eprintln!("\x1b[31m❌ [input] Error:\x1b[0m Failed to send expansion to typist: {}", e);
                                            }
                                        });
                                    }
                                    InputEvent::AiFix(prompt) => {
                                        if !AI_IN_FLIGHT.swap(true, Ordering::SeqCst) {
                                            let ai_config = config.ai.clone().unwrap();
                                            thread::spawn(move || {
                                                struct InFlightGuard;
                                                impl Drop for InFlightGuard {
                                                    fn drop(&mut self) {
                                                        AI_IN_FLIGHT.store(false, Ordering::SeqCst);
                                                    }
                                                }
                                                let _guard = InFlightGuard;
                                                if let Err(e) = ai::trigger_ai_fix(&prompt, &ai_config) {
                                                    eprintln!("\x1b[31m❌ [ai] Error:\x1b[0m {}", e);
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut i = pollfds.len();
        while i > 0 {
            i -= 1;
            if pollfds[i].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                eprintln!("\x1b[33m⚠️  [input]\x1b[0m Keyboard disconnected (path: {:?}), removing", keyboards[i].0);
                keyboards.remove(i);
            }
        }
        if keyboards.is_empty() {
            eprintln!("\x1b[31m❌ [input]\x1b[0m All keyboards disconnected, exiting");
            process::exit(1);
        }
    }
}
