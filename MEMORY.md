# Project: text_expander

## Overview
A minimal Wayland text expander compatible with the espanso configuration format, built in Rust using evdev and ydotool/wtype.

## Structure
```
text_expander/
├── Cargo.toml
├── install.sh
├── uninstall.sh
├── README.md
├── src/
│   ├── main.rs       # Entry point: device polling loop & hotplug logic
│   ├── lib.rs        # Library crate root exposing internal modules
│   ├── ai.rs         # AI completion via OpenAI-compatible endpoints using curl
│   ├── config.rs     # Config loaders & espanso YAML parsing logic
│   ├── inject.rs     # Clipboard & typing helpers (wl-copy, wl-paste, wtype, ydotool)
│   └── input.rs      # Input buffering, keycode mapping, & trigger expansion logic
└── tests/
    ├── ai_tests.rs      # Comprehensive unit/integration tests for the AI module
    ├── config_tests.rs  # Unit tests for variable expansions (date, echo)
    ├── inject_tests.rs  # Unit tests for keyboard conflicts
    └── input_tests.rs   # Unit tests for input tracking, Hotkey parsing, and text expansion logic
```

## Conventions
- Use minimum code necessary.
- Expose modules from `lib.rs` for testing.
- No tests inside the `src/` directory. All tests live under the `tests/` directory.

## Dependencies & Setup
- Requires read/write access to `/dev/input/*`.
- Needs `wl-copy`, `wl-paste` for clipboard operations.
- Needs `wtype` or `ydotool` for fake keystroke injection.
- Run `sudo ./install.sh` to install systemd service.

## Critical Information
- Integration tests for AI use the environment variable `TEXT_EXPANDER_AI_KEY`.
- Live integration tests are gated under `#[ignore]`.

## Blunders
- 2026-07-16: Placed AI and input unit tests inside `src/ai.rs` and `src/input.rs`, which cluttered production code. Exposing the library crate `lib.rs` and moving all tests to `tests/ai_tests.rs` and `tests/input_tests.rs` resolved this.
- 2026-07-16: Fixed overlapping clipboard expansion race condition, modifier wait handling, NSS home directory resolution, keypad desync issues, and asynchronous cancellation of shell expansions.
- 2026-09-05: Fixed KEY_COUNT incrementing on key release (val == 0) and modifier keys, which caused false-positive cancellation of async shell expansions like ;ip.
- 2026-09-05: Fixed key autorepeat handling (val == 2) for Backspace and typing buffer sync; suppressed repeat on CapsLock toggle and AI hotkeys.
- 2026-09-05: Fixed paired modifier desync (e.g. holding right shift while releasing left shift) by tracking left and right modifiers independently.
- 2026-09-05: Fixed expansion backspace character count (using trig.chars().count() instead of trig.len()), UTF-8 safe buffer drain, and word boundary index lookup.
- 2026-09-05: Fixed AI text replacement clipboard race by calling simulate_paste directly instead of type_expansion.
- 2026-09-05: Included HOME in get_wayland_env() and prioritized wayland-0 socket discovery.
- 2026-09-05: Replaced starter trigger ;datetime with ;timestamp.

