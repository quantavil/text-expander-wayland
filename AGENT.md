# AGENT.md

## Overview
- Minimal Wayland text expander reading espanso-format configs, using evdev and wtype/ydotool.
- Daemon runs as root systemd service (`text_expander.service`); user config in `~/.config/text_expander/base.yml`.

## Structure
- `src/main.rs`: Device polling loop, evdev event handling, keyboard hotplugging, expansion dispatch.
- `src/input.rs`: Keystroke buffer, modifier tracking, hotkey matching, trigger evaluation, char counts.
- `src/inject.rs`: Text injection via wtype/ydotool, clipboard preservation, backspace simulation.
- `src/config.rs`: YAML config loader, variable evaluation (date, shell, clipboard, echo), environment setup.
- `src/ai.rs`: OpenAI-compatible completions API via curl for text transformation hotkeys.
- `tests/`: Separate integration test suites (`input_tests.rs`, `inject_tests.rs`, `config_tests.rs`, `ai_tests.rs`).

## Conventions
- Minimum code that solves the problem. No tests inside `src/`; all tests in `tests/`.
- Build: `cargo build --release`. Deploy: `echo "admin0" | sudo -S ./install.sh`. Test: `cargo test`.

## Blunders
- 2026-07-16: Placed tests in `src/` files. Fix: Exposed crate modules via `src/lib.rs` and moved all tests to `tests/`.
- 2026-07-16: Clipboard restore collision during rapid expansions. Fix: Added `ACTIVE_CLIPBOARD_EXPANSIONS` counter and mutex guard.
- 2026-09-05: `KEY_COUNT` incremented on key release (`val == 0`) & modifiers. Fix: Increment only on non-modifier press/repeat (`val == 1 || val == 2`).
- 2026-09-05: Autorepeat omitted from `expander.process`. Fix: Process `val == 2` with `pressed: true` so held backspace/keys sync buffer.
- 2026-09-05: Paired modifiers desynced on overlapping release. Fix: Track `left_*` and `right_*` independently and OR their states.
- 2026-09-05: CapsLock and AI hotkeys retriggered on key repeat. Fix: Detect repeat via `keys_down` set and gate toggles.
- 2026-09-05: Backspace count used byte length `trig.len()`. Fix: Use `trig.chars().count()` to prevent deleting extra chars.
- 2026-09-05: Buffer drain byte index panic on multi-byte UTF-8. Fix: Use `char_indices()` to align cut offset to char boundary.
- 2026-09-05: Word boundary check used byte offset in `chars().nth()`. Fix: Sliced prefix and checked `chars().next_back()`.
- 2026-09-05: `ai.rs` called `type_expansion` which raced its own clipboard restore. Fix: Call `simulate_paste()` directly.
- 2026-09-05: `get_wayland_env()` omitted `HOME`. Fix: Added `HOME` fallback and prioritized `wayland-0` socket.
- 2026-09-05: Starter trigger `;datetime` renamed. Fix: Updated to `;ts` across install scripts and configs.
