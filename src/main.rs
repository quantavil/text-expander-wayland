mod config;
mod daemon;
mod inject;
mod input;

use evdev::{EventType, KeyCode};
use std::{
    env,
    os::unix::io::AsRawFd,
    process,
    thread,
    time::Duration,
};

use config::load_configs;
use daemon::daemonize;
use inject::type_expansion;
use input::{find_keyboards, TextExpander};

fn main() {
    let args: Vec<String> = env::args().collect();
    let daemon_mode = args.iter().any(|a| a == "-d" || a == "--daemon");

    eprintln!("\x1b[1m🚀 [text_expander]\x1b[0m lightweight espanso replacement for Wayland");

    let triggers = load_configs();
    if triggers.is_empty() {
        eprintln!("\x1b[31m❌ [config] Error:\x1b[0m No triggers loaded. Create config in ~/.config/text_expander/");
        process::exit(1);
    }
    eprintln!("\x1b[32m📦 [config]\x1b[0m Loaded {} triggers total", triggers.len());

    let mut keyboards = find_keyboards();
    if keyboards.is_empty() {
        eprintln!("\x1b[31m❌ [input] Error:\x1b[0m No keyboards found. Need read access to /dev/input/*");
        process::exit(1);
    }

    if daemon_mode {
        eprintln!("\x1b[34mℹ️  [daemon]\x1b[0m Daemonizing...");
        daemonize();
    } else {
        eprintln!("\x1b[32m🟢 [daemon]\x1b[0m Ready! (use -d/--daemon to run in background)");
    }

    let mut expander = TextExpander::new(triggers);

    let (tx, rx) = std::sync::mpsc::channel::<(usize, config::Trigger, KeyCode)>();
    thread::spawn(move || {
        while let Ok((n, trigger, last_key)) = rx.recv() {
            thread::sleep(Duration::from_millis(10));
            let text = trigger.expand();
            type_expansion(n, &text, last_key);
        }
    });

    loop {
        let raw_fds: Vec<i32> = keyboards.iter().map(|(_, k)| k.as_raw_fd()).collect();
        let mut pollfds: Vec<libc::pollfd> = raw_fds.iter()
            .map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            .collect();

        let poll_res = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 5000) };
        if poll_res < 0 {
            continue;
        }

        if poll_res == 0 {
            // Timeout reached: scan for new keyboards (hotplugging support)
            let scanned = find_keyboards();
            for (path, device) in scanned {
                if !keyboards.iter().any(|(p, _)| *p == path) {
                    eprintln!("\x1b[36m🔌 [input]\x1b[0m New keyboard hotplugged: {:?}", path);
                    keyboards.push((path, device));
                }
            }
            continue;
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
            process::exit(0);
        }

        let ready: Vec<usize> = pollfds.iter().enumerate()
            .filter(|(_, p)| p.revents & libc::POLLIN != 0)
            .map(|(i, _)| i).collect();

        for &i in &ready {
            if i >= keyboards.len() { continue }
            if let Ok(events) = keyboards[i].1.fetch_events() {
                for ev in events {
                    if ev.event_type() == EventType::KEY {
                        if let Some((n, trigger)) = expander.process(KeyCode::new(ev.code()), ev.value() == 1) {
                            let _ = tx.send((n, trigger, KeyCode::new(ev.code())));
                        }
                    }
                }
            }
        }
    }
}
