#!/bin/bash
set -e

# Configuration
REAL_USER=${SUDO_USER:-$(whoami)}
REAL_UID=${SUDO_UID:-$(id -u)}
REAL_HOME=$(getent passwd "$REAL_USER" | cut -d: -f6)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_SRC="$SCRIPT_DIR/target/release/text_expander"
BINARY_DST="/usr/local/bin/text_expander"
CONFIG_DIR="$REAL_HOME/.config/text_expander"
SERVICE_PATH="/etc/systemd/system/text_expander.service"

echo "=== Installing text_expander ==="

# 1. Copy the binary
if [ -f "$BINARY_SRC" ]; then
    if systemctl is-active --quiet text_expander 2>/dev/null; then
        echo "Stopping active text_expander service..."
        systemctl stop text_expander || true
    fi
    # Kill any stray processes not managed by systemd
    pkill -x text_expander 2>/dev/null || true
    sleep 0.5
    echo "Copying binary to $BINARY_DST..."
    rm -f "$BINARY_DST"
    cp "$BINARY_SRC" "$BINARY_DST"
    chmod +x "$BINARY_DST"
else
    echo "Error: Release binary not found at $BINARY_SRC."
    echo "Please build the project first with: cargo build --release"
    exit 1
fi

# 2. Setup config directory and starter base.yml
echo "Setting up configuration directory at $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_DIR/base.yml" ]; then
    echo "Creating starter configuration in $CONFIG_DIR/base.yml..."
    cat << 'EOF' > "$CONFIG_DIR/base.yml"
matches:
  # Simple replacement
  - trigger: ";sig"
    replace: "Best regards,\nquantavil"

  # Date variable
  - trigger: ";date"
    replace: "{{date}}"
    vars:
      - name: date
        type: date
        params:
          format: "%Y-%m-%d"

  # Time variable
  - trigger: ";time"
    replace: "{{time}}"
    vars:
      - name: time
        type: date
        params:
          format: "%H:%M"

  # Date and Time variable
  - trigger: ";datetime"
    replace: "{{dt}}"
    vars:
      - name: dt
        type: date
        params:
          format: "%Y-%m-%d %H:%M:%S"

  # Get external IP address
  - trigger: ";ip"
    replace: "{{ip}}"
    vars:
      - name: ip
        type: shell
        params:
          cmd: "curl -s ifconfig.me"

  # Print current system details
  - trigger: ";sysinfo"
    replace: "{{sysinfo}}"
    vars:
      - name: sysinfo
        type: shell
        params:
          cmd: "uname -sr"

  # Paste from clipboard
  - trigger: ";clip"
    replace: "{{clip}}"
    vars:
      - name: clip
        type: clipboard

  # List all triggers
  - trigger: ";?"
    replace: "Available commands:\n{{commands}}"
    vars:
      - name: commands
        type: shell
        params:
          cmd: |-
            grep -r -h -E '[t]riggers?:' ~/.config/text_expander/ | sed -E 's/.*triggers?:\s*//; s/,/\n/g' | tr -d '[]\" ' | sort -u

  - trigger: ";improve"
    replace: |-
      Audit this codebase like a principal engineer, not an assistant. Rules:

      1. RECON: map the stack, conventions, and the actual build/test/lint commands. 
         Don't guess these — find them in package.json/Makefile/CI config.

      2. AUDIT across these categories: correctness, security, performance, 
         test coverage, tech debt, dependency/migration risk, DX, docs, and 
         direction (feature gaps — must cite real evidence in repo, no generic advice).
         For each finding: file:line, impact, effort (S/M/L), confidence (LOW/MED/HIGH).

      3. VET yourself: re-check every finding you just made against the actual file 
         before reporting it. Drop false positives. If something looks like a finding 
         but is by-design (e.g. standard pattern), say so and exclude it, with reason.

      4. PRIORITIZE: rank by (impact / effort), weighted by confidence. Output a table.

      5. STOP THERE. Do not write code yet. Show me the table, I'll tell you 
         which numbers to turn into plans.

      Once I pick, write ONE plan per finding as a separate self-contained doc:
      - exact current-state code excerpt
      - exact steps to change it
      - the repo's own verified test/lint commands as pass/fail gates
      - explicit out-of-scope list
      - STOP conditions: what to do if reality doesn't match this plan

      Hard rules: never touch source code in this pass. Never invent secret values — 
      name the location and credential type only, always recommend rotation if found.
      If I ask you to just implement instead of planning, ask me to confirm I want 
      to skip the plan step first.

  - trigger: ";improveplan"
    replace: |-
      Skip audit. Write ONE self-contained plan for: [DESCRIBE THE THING].
      Must include: current-state code excerpt, exact steps, this repo's own 
      test/lint commands as verification gates, explicit out-of-scope list, 
      STOP conditions if reality doesn't match assumptions. Do not implement it.

# AI text processing hotkeys
ai:
  api_key: "your-gemini-api-key"
  endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions" # Optional, defaults to Gemini
  model: "gemini-3.1-flash-lite" # Optional, defaults to gemini-3.1-flash-lite
  matches:
    # Fix grammar (select text → corrected text)
    - hotkey: "ctrl+alt+f"
      prompt: "You are a professional editor. Correct any grammar, spelling, or punctuation errors in the text below. Keep the original meaning and tone. Do not use em dashes. Respond ONLY with the corrected text and absolutely nothing else. Do not add quotes, explanations, or markdown formatting."

    # Make concise (select verbose text → shorter version)
    - hotkey: "ctrl+alt+s"
      prompt: "You are a concise communication expert. Rewrite the text below to be as short as possible while keeping all essential meaning. Remove filler words, redundancy, and fluff. Use short direct sentences. Do not use em dashes. Respond ONLY with the rewritten text. No quotes, explanations, or markdown."

    # Generate commit message (select description of changes → conventional commit)
    - hotkey: "ctrl+alt+m"
      prompt: "You are a Git expert. Generate a Conventional Commits message from the description below. Format: <type>(<optional scope>): <description>. Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore. Use imperative mood. Keep subject under 50 chars. Respond ONLY with the commit message. No markdown, no quotes, no explanation."

    # Draft a reply (select someone's message → your reply)
    - hotkey: "ctrl+alt+r"
      prompt: "You are a professional communicator. Draft a concise, friendly reply to the message below. Be helpful and direct. Match the formality level of the original message. Keep it brief. Do not use em dashes. Respond ONLY with the reply text. No quotes, explanations, subject lines, or markdown."

    # Improve prompt (select rough prompt → polished detailed prompt)
    - hotkey: "ctrl+alt+p"
      prompt: "You are an expert prompt engineer. Rewrite the rough prompt below into a clear, detailed, and well-structured instruction that will get better results from an AI model. Add specificity, constraints, and output format where missing. Keep the original intent. Respond ONLY with the improved prompt. No quotes, explanations, or markdown formatting."

    # Explain code/error (select code or error → plain English explanation)
    - hotkey: "ctrl+alt+e"
      prompt: "You are a senior developer and great teacher. Explain the code or error message below in plain English. Be concise. Say what it does, why the error happens if applicable, and how to fix it. Avoid unnecessary jargon. Respond ONLY with the explanation. No markdown formatting, no code blocks."
EOF
fi

# Ensure correct ownership of the config directory and files so the user can edit them
chown -R "$REAL_USER:$REAL_USER" "$CONFIG_DIR"

# 3. Create systemd service
echo "Creating systemd service at $SERVICE_PATH..."
cat << EOF > "$SERVICE_PATH"
[Unit]
Description=Text Expander Daemon
After=graphical.target

[Service]
ExecStart=$BINARY_DST
Restart=always
Environment=SUDO_USER=$REAL_USER
Environment=SUDO_UID=$REAL_UID

[Install]
WantedBy=graphical.target
EOF

# 4. Reload and start systemd service
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo "Enabling and starting text_expander service..."
systemctl enable text_expander
systemctl restart text_expander

echo "Service status:"
systemctl status text_expander --no-pager || true

echo "=== Installation complete! ==="
echo "You can edit your triggers in $CONFIG_DIR/base.yml"
echo "After editing, run: systemctl restart text_expander"
