# text_expander

Lightweight text expander for Wayland. Built as a minimal replacement for [espanso](https://espanso.org/) that reads espanso-format config files.

It is extremely resource-efficient, consuming only **~1.7 MB - 3.4 MB** of RAM at runtime (compared to Espanso's 50MB - 100MB+).

### Comparison with Espanso

| Feature | `text_expander` | `espanso` |
|---------|-----------------|-----------|
| **Memory Usage** | **~1.7 MB - 3.4 MB RAM** | ~50 MB - 120 MB RAM |
| **Process Model** | Single minimal daemon | Multiple daemon & worker processes |
| **Wayland Integration** | Native (via `ydotool` / `wtype`) | Native/XWayland (often requires complex setups) |
| **Config Format** | Espanso-compatible YAML | YAML |
| **Basic Triggers & Vars** | Supported (date, shell, clipboard) | Supported |
| **Advanced Features** | No (Regex triggers, forms, scripts) | Supported |
| **System Overhead** | Negligible | Moderate |

*Note: Memory usage verified on active daemon via `ps aux` showing RSS of ~2.6 MB.*

Supports the most commonly used espanso match features (simple triggers, variables, shell commands). Advanced features like regex triggers, forms, and app-specific configs are not supported.

## Requirements

- Linux + Wayland
- `wtype` or `ydotool` (text injection; `ydotool` is preferred if running)
- `wl-paste` (clipboard variable support)
- Root access for `/dev/input/event*`

## Installation & Updating

To build, install, or update the daemon:

```bash
# Build the binary
cargo build --release

# Run the install script (requires root/sudo)
sudo ./install.sh
```

The `install.sh` script automatically:
1. Stops the running daemon service if active.
2. Copies the compiled release binary to `/usr/local/bin/text_expander`.
3. Creates and configures the systemd service at `/etc/systemd/system/text_expander.service` (setting up the correct user privileges, environments, and home directory).
4. Reloads systemd configuration, enables, and starts the background service.

Your trigger configurations will remain preserved in `~/.config/text_expander/`.

## Usage

```bash
text_expander                    # Run (foreground)
text_expander --list-triggers    # List all loaded triggers and exit
text_expander --version          # Show version
text_expander --help             # Show help
```

## Config

Location: `~/.config/text_expander/`

All `.yml` and `.yaml` files are loaded recursively.

### Syntax (espanso-compatible)

```yaml
matches:
  # Simple replacement
  - trigger: ":sig"
    replace: "Best regards,\nJohn"

  # Multiple triggers for one replacement
  - triggers: [":hi", ":hello"]
    replace: "Hello there!"

  # Date variable
  - trigger: ":date"
    replace: "{{date}}"
    vars:
      - name: date
        type: date
        params:
          format: "%Y-%m-%d"

  # Shell command
  - trigger: ":ip"
    replace: "{{ip}}"
    vars:
      - name: ip
        type: shell
        params:
          cmd: "curl -s ifconfig.me"

  # Clipboard
  - trigger: ":paste"
    replace: "{{clip}}"
    vars:
      - name: clip
        type: clipboard
```

### Variable Types

| Type | Params | Description |
|------|--------|-------------|
| `date` | `format` | strftime format string |
| `shell` | `cmd` | Shell command output |
| `clipboard` | - | Current clipboard content (via `wl-paste`) |
| `echo` | `echo` | Static text |

### Supported espanso Features

- `trigger` (single string) and `triggers` (array of strings)
- `replace` with `{{variable}}` interpolation
- `vars` with `date`, `shell`, `clipboard`, and `echo` types
- `global_vars` for shared variables across matches
- Recursive YAML file loading
- Cursor hints/placement (`$|$`)

### Not Supported

These espanso features are intentionally out of scope for this minimal tool:

- Regex triggers, word boundaries, case propagation
- Forms, choice dialogs
- Rich text (markdown/HTML), image pasting
- App-specific configs, toggle key, search bar
- Config options (backend, clipboard_threshold, etc.)
- `random`, `script`, `match` variable types

## Migrating from espanso

```bash
# Stop espanso
systemctl --user stop espanso

# Copy config
mkdir -p ~/.config/text_expander
cp -r ~/.config/espanso/* ~/.config/text_expander/

# Remove espanso (optional)
rm -rf ~/.config/espanso
```

Simple trigger/replace matches and basic variable types will work as-is. Matches using unsupported features (regex, forms, etc.) will be silently skipped.

## How It Works

1. Reads keyboard input via evdev (prefers virtual keyboards like keyd/kmonad)
2. Buffers keystrokes and matches against triggers
3. On match: sends backspaces to delete trigger, types replacement via `wtype`

## Managing the Service

The installation script configures `text_expander` as a systemd service. You can manage it using standard systemctl commands:

```bash
# Restart the daemon (e.g. after modifying triggers)
sudo systemctl restart text_expander

# Check service logs and status
sudo systemctl status text_expander
journalctl -u text_expander -f
```

## License

[GPL-3.0](LICENSE)
