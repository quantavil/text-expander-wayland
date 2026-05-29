# Project: text_expander

## Overview
A lightweight, minimal text expander for Wayland written in Rust. It serves as a replacement for Espanso by reading Espanso-compatible config files (YAML). It works by directly reading keyboard events from `/dev/input/event*` devices, matching input buffers against triggers, and injecting the expanded text using `wtype` (via Wayland environment variables). It runs as a daemon or in the foreground and requires root access.

## Structure
text_expander/
├── Cargo.toml            # Project manifest & dependency configuration
├── Cargo.lock            # Lockfile for Rust dependencies
├── LICENSE               # GPL-3.0 License
├── README.md             # Overview, setup, usage instructions, systemd service configuration
├── install.sh            # Setup script for automated installation and daemon service configuration
├── uninstall.sh          # Cleanup script to stop/disable the service and uninstall the binary
└── src/
    └── main.rs           # Core implementation (CLI, config loading, input polling, key-to-char mapping, expansion logic)

## Conventions
- Single-file codebase: The core expansion logic, EVDEV event loop, and Espanso YAML parser are currently all located in `src/main.rs`.
- Configuration path resolving: Automatically looks for `$HOME/.config/text_expander` (falling back to parsing `/etc/passwd` via `SUDO_USER` if run under `sudo`).
- Event polling: Uses Linux system call `libc::poll` to poll all active `/dev/input/event*` devices synchronously.
- Text injection: Relies on invoking the external command `wtype` to simulate key presses (including backspaces) and send UTF-8 characters.

## Dependencies & Setup
- Requires Rust/Cargo to compile.
- Requires `wtype` and `wl-paste` to be installed on the system.
- Requires root privileges (`sudo`) to read from `/dev/input/*`.
- Rust dependencies: `evdev` (0.12) for handling input events, `serde` and `serde_yaml` for config loading, and `libc` (0.2) for raw system functions.

## Critical Information
- **Root Permissions:** Since it reads directly from `/dev/input/event*`, it must run with root permissions. If run via `sudo`, it retrieves the real user's environment to launch `wtype` via the user's Wayland socket.
- **Wayland Compatibility:** Injection uses `wtype`, so it only works natively on Wayland compositors that support virtual keyboard protocols.
- **Keyboard Hook Interferences:** If tools like keyd/kmonad are present, it tries to detect their virtual keyboards and read from them to prevent double-expansion or missed events.

## Insights
- Simple Espanso compatibility makes it easy to migrate configs directly.
- The use of `libc::poll` makes it extremely lightweight compared to heavy daemons.
- **Bug Fix (Keyboard Detection):** The virtual keyboard detection logic was refactored to check specifically for remappers like `keyd`, `kmonad`, or `kanata`. Previously, the presence of any device containing "virtual" in its name (e.g. `ydotoold virtual device`) would disable reading from physical keyboards entirely.
- **Bug Fix (Clipboard Paste Injection):** Added clipboard-based pasting via simulated Ctrl+V/Shift+Insert for expansions that are multiline, long (>25 chars), or equal to the clipboard. This fixes capital character dropping (e.g., Phase -> hase) caused by physical key conflicts and prevents editor auto-indent/auto-bracket malfunctions when typing code.

## Blunders
*(None logged yet)*

