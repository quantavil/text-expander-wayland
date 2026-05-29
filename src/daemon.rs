use std::{fs, os::unix::io::AsRawFd, process};

pub fn daemonize() {
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
