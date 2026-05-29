#!/bin/bash
set -e

# Configuration
REAL_USER=${SUDO_USER:-$(whoami)}
REAL_HOME=$(getent passwd "$REAL_USER" | cut -d: -f6)
BINARY_DST="/usr/local/bin/text_expander"
SERVICE_PATH="/etc/systemd/system/text_expander.service"
CONFIG_DIR="$REAL_HOME/.config/text_expander"

echo "=== Uninstalling text_expander ==="

# 1. Stop and disable service
if systemctl list-unit-files | grep -q "text_expander.service"; then
    echo "Stopping and disabling systemd service..."
    systemctl stop text_expander || true
    systemctl disable text_expander || true
fi

# 2. Remove systemd service file
if [ -f "$SERVICE_PATH" ]; then
    echo "Removing service file: $SERVICE_PATH"
    rm "$SERVICE_PATH"
    systemctl daemon-reload
fi

# 3. Remove binary
if [ -f "$BINARY_DST" ]; then
    echo "Removing binary: $BINARY_DST"
    rm "$BINARY_DST"
fi

# 4. Handle configuration directory
if [ -d "$CONFIG_DIR" ]; then
    echo "Purging configuration directory: $CONFIG_DIR"
    rm -rf "$CONFIG_DIR"
fi

echo "=== Uninstallation complete! ==="
