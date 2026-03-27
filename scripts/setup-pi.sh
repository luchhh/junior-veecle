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
if [ -f "$RUNNER_ENV" ] && ! grep -q "cargo" "$RUNNER_ENV"; then
    echo "==> Adding cargo to GitHub Actions runner PATH..."
    echo "PATH=$HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" >> "$RUNNER_ENV"
    sudo systemctl restart "actions.runner.luchhh-junior-veecle.$(hostname).service" || true
elif [ ! -f "$RUNNER_ENV" ]; then
    echo "==> Runner not installed yet — remember to add cargo to PATH after setup:"
    echo "    echo 'PATH=$HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin' >> ~/actions-runner/.env"
fi

# --- Done ---
echo "==> Done. Start Junior with: sudo systemctl start junior"
echo "    Watch logs with:          journalctl -u junior -f"
