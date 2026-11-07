#!/usr/bin/env bash
# Setup script for Junior on a fresh Raspberry Pi 5 (64-bit Debian)
# Run as the robot user (not root): bash scripts/setup-pi.sh

set -euo pipefail

REPO_URL="https://github.com/luchhh/junior-veecle.git"
REPO_DIR="$HOME/junior-veecle"
USER="$(whoami)"

echo "==> Setting up Junior on $(hostname) as user $USER"

# --- Rust ---
if ! command -v "$HOME/.cargo/bin/rustc" &>/dev/null; then
    echo "==> Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
else
    echo "==> Rust already installed: $("$HOME/.cargo/bin/rustc" --version)"
fi

# --- Repo ---
if [ ! -d "$REPO_DIR" ]; then
    echo "==> Cloning repo..."
    git clone "$REPO_URL" "$REPO_DIR"
else
    echo "==> Repo already cloned at $REPO_DIR"
fi

# --- Initial build ---
echo "==> Building Junior (this may take a few minutes on first run)..."
cd "$REPO_DIR" && "$HOME/.cargo/bin/cargo" build

# --- .env ---
if [ ! -f "$REPO_DIR/.env" ]; then
    echo ""
    echo "==> No .env file found. Create $REPO_DIR/.env with your secrets:"
    echo "    OPENAI_API_KEY=sk-..."
    echo ""
fi

# --- UART for the LED face (STM32 on GPIO14 TX / GPIO15 RX) ---
# On the Pi 5 this is /dev/ttyAMA0; /dev/serial0 points at the debug connector, so the
# serial console there doesn't get in the way.
BOOT_CONFIG="/boot/firmware/config.txt"
if ! grep -q "^dtparam=uart0=on" "$BOOT_CONFIG"; then
    echo "==> Enabling UART0 on GPIO14/15 (reboot required)..."
    echo "dtparam=uart0=on" | sudo tee -a "$BOOT_CONFIG" > /dev/null
fi
if [ -f "$REPO_DIR/.env" ] && ! grep -q "^DISPLAY_SERIAL=" "$REPO_DIR/.env"; then
    echo "DISPLAY_SERIAL=/dev/ttyAMA0" >> "$REPO_DIR/.env"
fi
sudo usermod -aG dialout "$USER"

# --- systemd service ---
echo "==> Installing junior systemd service..."
sudo tee /etc/systemd/system/junior.service > /dev/null << EOF
[Unit]
Description=Junior Robot
After=network.target sound.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$REPO_DIR
ExecStart=$HOME/.cargo/bin/cargo run
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable junior

# --- sudoers ---
echo "==> Configuring passwordless sudo for service management..."
echo "$USER ALL=(ALL) NOPASSWD: /bin/systemctl restart junior, /bin/systemctl stop junior, /bin/systemctl start junior" \
    | sudo tee /etc/sudoers.d/junior > /dev/null

# --- Audio levels ---
echo ""
echo "==> Audio levels — adjust manually if needed:"
echo "    Speaker (card 0, USB audio out):"
echo "      amixer -c 0 sset PCM 88%"
echo "    Mic (card 2, USB PnP mic):"
echo "      amixer -c 2 sset Mic 81%"
echo "      amixer -c 2 sset 'Auto Gain Control' off  # optional: disable AGC"
echo "    Interactive mixer: alsamixer -c 0  /  alsamixer -c 2"
echo ""

# --- GitHub Actions runner ---
echo ""
echo "==> GitHub Actions runner setup"
echo "    Follow the instructions at:"
echo "    https://github.com/luchhh/junior-veecle/settings/actions/runners/new"
echo "    Install to: $HOME/actions-runner"
echo ""

# Add cargo to PATH in runner environment so it's available in non-interactive shells
RUNNER_ENV="$HOME/actions-runner/.env"
RUNNER_SVC="actions.runner.luchhh-junior-veecle.$(hostname).service"
if [ -f "$RUNNER_ENV" ] && ! grep -q "cargo" "$RUNNER_ENV"; then
    echo "==> Adding cargo to GitHub Actions runner PATH..."
    echo "PATH=$HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" >> "$RUNNER_ENV"
    sudo systemctl restart "$RUNNER_SVC" || true
elif [ ! -f "$RUNNER_ENV" ]; then
    echo "==> Runner not installed yet — remember to add cargo to PATH after setup:"
    echo "    echo 'PATH=$HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin' >> ~/actions-runner/.env"
fi

# Patch runner service to restart automatically when the GitHub session expires
RUNNER_SVC_FILE="/etc/systemd/system/$RUNNER_SVC"
if [ -f "$RUNNER_SVC_FILE" ] && ! grep -q "Restart=always" "$RUNNER_SVC_FILE"; then
    echo "==> Patching runner service with Restart=always..."
    sudo sed -i '/TimeoutStopSec=5min/a Restart=always\nRestartSec=30' "$RUNNER_SVC_FILE"
    sudo systemctl daemon-reload
    sudo systemctl restart "$RUNNER_SVC" || true
fi

# --- WiFi watchdog ---
echo "==> Installing WiFi watchdog..."
sudo tee /usr/local/bin/wifi-watchdog.sh > /dev/null << 'WATCHDOG'
#!/bin/bash

bounce() {
    /sbin/ip link set wlan0 down
    sleep 2
    /sbin/ip link set wlan0 up
}

# No IP — not connected to anything
if ! /sbin/ip addr show wlan0 | grep -q 'inet '; then
    bounce
    exit 0
fi

# Has IP but no internet
if ! ping -c 1 -W 3 8.8.8.8 &>/dev/null; then
    bounce
fi
WATCHDOG

sudo chmod +x /usr/local/bin/wifi-watchdog.sh
(sudo crontab -l 2>/dev/null | grep -v wifi-watchdog; echo '* * * * * /usr/local/bin/wifi-watchdog.sh') | sudo crontab -

# --- Tailscale ---
if ! command -v tailscale &>/dev/null; then
    echo "==> Installing Tailscale..."
    curl -fsSL https://tailscale.com/install.sh | sh
    echo ""
    echo "==> Authenticate Tailscale by running: sudo tailscale up"
    echo "    Then install Tailscale on your phone and log in with the same account."
    echo "    The Pi will be reachable at http://juniorpi:5000 from any device on your Tailscale network."
    echo ""
else
    echo "==> Tailscale already installed: $(tailscale version)"
fi

# --- Mic volume server ---
echo "==> Installing mic volume server..."
sudo apt-get install -y python3-flask > /dev/null
sudo cp "$REPO_DIR/scripts/mic-volume-server.py" /usr/local/bin/mic-volume-server.py
sudo tee /etc/systemd/system/mic-volume.service > /dev/null << EOF
[Unit]
Description=Junior Mic Volume Control
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/python3 /usr/local/bin/mic-volume-server.py
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable mic-volume
sudo systemctl start mic-volume

# --- mem0 memory server ---
echo "==> Installing mem0 memory server..."
pip3 install --quiet --break-system-packages mem0ai fastapi uvicorn 2>/dev/null \
    || pip3 install --quiet mem0ai fastapi uvicorn
sudo cp "$REPO_DIR/scripts/mem0-server.py" /usr/local/bin/mem0-server.py
sudo tee /etc/systemd/system/junior-mem0.service > /dev/null << EOF
[Unit]
Description=Junior mem0 Memory Server
After=network.target

[Service]
Type=simple
User=$USER
EnvironmentFile=$REPO_DIR/.env
ExecStart=/usr/bin/python3 /usr/local/bin/mem0-server.py
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable junior-mem0
sudo systemctl start junior-mem0

# --- Wake-word listener (local speech-to-text while Junior sleeps) ---
echo "==> Installing wake-word listener..."
pip3 install --quiet --break-system-packages faster-whisper fastapi uvicorn 2>/dev/null \
    || pip3 install --quiet faster-whisper fastapi uvicorn
sudo cp "$REPO_DIR/scripts/wake-server.py" /usr/local/bin/wake-server.py
sudo tee /etc/systemd/system/junior-wake.service > /dev/null << EOF
[Unit]
Description=Junior Wake-Word Listener
After=network.target

[Service]
Type=simple
User=$USER
ExecStart=/usr/bin/python3 /usr/local/bin/wake-server.py
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable junior-wake
sudo systemctl start junior-wake

# --- Done ---
echo "==> Done. Start Junior with: sudo systemctl start junior"
echo "    Watch logs with:          journalctl -u junior -f"
echo "    Mic volume UI:            http://juniorpi:5000  (requires Tailscale)"
echo "    mem0 memories:            curl http://localhost:8000/v1/memories/?user_id=junior"
