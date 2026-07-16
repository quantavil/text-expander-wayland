use std::{fs, os::unix::io::AsRawFd, process};

pub fn daemonize() {
    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("First fork failed");
            process::exit(1);
        }
        0 => {}
        _ => process::exit(0),
    }

    if unsafe { libc::setsid() } == -1 {
        eprintln!("setsid failed");
        process::exit(1);
    }

    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("Second fork failed");
            process::exit(1);
        }
        0 => {}
        _ => process::exit(0),
    }

    if unsafe { libc::chdir(c"/".as_ptr()) } != 0 {
        eprintln!("chdir failed");
        process::exit(1);
    }

    if let Ok(devnull) = fs::OpenOptions::new().read(true).write(true).open("/dev/null") {
        let fd = devnull.as_raw_fd();
        unsafe {
            libc::dup2(fd, 0);
            libc::dup2(fd, 1);
            libc::dup2(fd, 2);
        }
    } else {
        eprintln!("Failed to open /dev/null");
        process::exit(1);
    }
}
