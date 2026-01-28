#!/bin/bash
# GCP DevBox VM Setup Script
# This runs inside the VM after creation

set -euo pipefail

# ============================================================================
# Configuration (passed via metadata)
# ============================================================================
REPO_URL="${REPO_URL:-}"
REPO_NAME="${REPO_NAME:-}"
POST_CLONE_COMMANDS="${POST_CLONE_COMMANDS:-}"
TAILSCALE_AUTHKEY="${TAILSCALE_AUTHKEY:-}"
NODE_VERSION="${NODE_VERSION:-20}"
INSTALL_DOCKER="${INSTALL_DOCKER:-true}"
INSTALL_RUST="${INSTALL_RUST:-true}"
INSTALL_GO="${INSTALL_GO:-true}"
GITHUB_TOKEN="${GITHUB_TOKEN:-}"
DEV_USER="${DEV_USER:-devbox}"

LOG_FILE="/var/log/devbox-setup.log"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "=========================================="
echo "GCP DevBox Setup - $(date)"
echo "=========================================="

# ============================================================================
# Create dev user
# ============================================================================
setup_user() {
    echo "[1/8] Setting up dev user: $DEV_USER"

    if ! id "$DEV_USER" &>/dev/null; then
        useradd -m -s /bin/bash "$DEV_USER"
        usermod -aG sudo "$DEV_USER"
        echo "$DEV_USER ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/$DEV_USER
    fi

    # Setup SSH for the user
    mkdir -p /home/$DEV_USER/.ssh
    cp /root/.ssh/authorized_keys /home/$DEV_USER/.ssh/ 2>/dev/null || true
    chown -R $DEV_USER:$DEV_USER /home/$DEV_USER/.ssh
    chmod 700 /home/$DEV_USER/.ssh
    chmod 600 /home/$DEV_USER/.ssh/authorized_keys 2>/dev/null || true
}

# ============================================================================
# Install base packages
# ============================================================================
install_base() {
    echo "[2/8] Installing base packages"

    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y \
        git curl wget build-essential \
        python3-pip python3-venv python3-full \
        jq unzip htop tmux \
        apt-transport-https ca-certificates \
        gnupg lsb-release software-properties-common
}

# ============================================================================
# Install Node.js
# ============================================================================
install_node() {
    echo "[3/8] Installing Node.js $NODE_VERSION"

    curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash -
    apt-get install -y nodejs

    # Install global npm packages
    npm install -g pnpm yarn @anthropic-ai/claude-code @claude-flow/cli@latest
}

# ============================================================================
# Install Docker
# ============================================================================
install_docker() {
    if [ "$INSTALL_DOCKER" != "true" ]; then
        echo "[4/8] Skipping Docker installation"
        return
    fi

    echo "[4/8] Installing Docker"

    curl -fsSL https://get.docker.com | sh
    usermod -aG docker $DEV_USER
    systemctl enable docker
    systemctl start docker
}

# ============================================================================
# Install Rust
# ============================================================================
install_rust() {
    if [ "$INSTALL_RUST" != "true" ]; then
        echo "[5/8] Skipping Rust installation"
        return
    fi

    echo "[5/8] Installing Rust"

    sudo -u $DEV_USER bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
}

# ============================================================================
# Install Go
# ============================================================================
install_go() {
    if [ "$INSTALL_GO" != "true" ]; then
        echo "[6/8] Skipping Go installation"
        return
    fi

    echo "[6/8] Installing Go"

    GO_VERSION="1.22.0"
    wget -q "https://go.dev/dl/go${GO_VERSION}.linux-amd64.tar.gz" -O /tmp/go.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf /tmp/go.tar.gz
    rm /tmp/go.tar.gz

    echo 'export PATH=$PATH:/usr/local/go/bin' >> /home/$DEV_USER/.bashrc
    echo 'export PATH=$PATH:$HOME/go/bin' >> /home/$DEV_USER/.bashrc
}

# ============================================================================
# Install Tailscale
# ============================================================================
install_tailscale() {
    echo "[7/8] Installing Tailscale"

    curl -fsSL https://tailscale.com/install.sh | sh

    if [ -n "$TAILSCALE_AUTHKEY" ]; then
        echo "Connecting to Tailscale..."
        tailscale up --authkey="$TAILSCALE_AUTHKEY" --ssh
        echo "Tailscale connected! SSH via: $(tailscale ip -4)"
    else
        echo "No Tailscale authkey provided. Run 'sudo tailscale up' manually."
    fi
}

# ============================================================================
# Clone and setup repository
# ============================================================================
setup_repo() {
    echo "[8/8] Setting up repository"

    if [ -z "$REPO_URL" ]; then
        echo "No repository URL provided, skipping clone"
        return
    fi

    WORKSPACE="/home/$DEV_USER/workspace"
    mkdir -p "$WORKSPACE"
    chown $DEV_USER:$DEV_USER "$WORKSPACE"

    cd "$WORKSPACE"

    # Clone with token if available
    if [ -n "$GITHUB_TOKEN" ]; then
        CLONE_URL=$(echo "$REPO_URL" | sed "s|https://|https://${GITHUB_TOKEN}@|")
    else
        CLONE_URL="$REPO_URL"
    fi

    if [ -n "$REPO_NAME" ]; then
        sudo -u $DEV_USER git clone "$CLONE_URL" "$REPO_NAME"
        cd "$REPO_NAME"
    else
        sudo -u $DEV_USER git clone "$CLONE_URL"
        cd "$(basename "$REPO_URL" .git)"
    fi

    # Run post-clone commands
    if [ -n "$POST_CLONE_COMMANDS" ]; then
        echo "Running post-clone commands..."
        echo "$POST_CLONE_COMMANDS" | while IFS= read -r cmd; do
            if [ -n "$cmd" ]; then
                echo "  > $cmd"
                sudo -u $DEV_USER bash -c "$cmd" || true
            fi
        done
    fi

    # Initialize claude-flow if package.json exists
    if [ -f "package.json" ]; then
        echo "Initializing claude-flow..."
        sudo -u $DEV_USER npx @claude-flow/cli@latest init --force || true
    fi
}

# ============================================================================
# Setup VS Code Server
# ============================================================================
setup_vscode_tunnel() {
    echo "Setting up VS Code CLI for tunnels..."

    cd /tmp
    curl -fsSL "https://code.visualstudio.com/sha/download?build=stable&os=cli-linux-x64" -o vscode-cli.tar.gz
    tar -xzf vscode-cli.tar.gz
    mv code /usr/local/bin/code-tunnel
    rm vscode-cli.tar.gz

    # Create systemd service for VS Code tunnel
    cat > /etc/systemd/system/code-tunnel.service << 'EOF'
[Unit]
Description=VS Code Tunnel
After=network.target

[Service]
Type=simple
User=devbox
ExecStart=/usr/local/bin/code-tunnel tunnel --accept-server-license-terms
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

    echo "VS Code tunnel service created. Start with: sudo systemctl start code-tunnel"
}

# ============================================================================
# Create helpful scripts
# ============================================================================
create_helper_scripts() {
    # Devbox status script
    cat > /usr/local/bin/devbox-status << 'EOF'
#!/bin/bash
echo "=== DevBox Status ==="
echo ""
echo "Tailscale IP: $(tailscale ip -4 2>/dev/null || echo 'Not connected')"
echo "Public IP: $(curl -s ifconfig.me)"
echo "Hostname: $(hostname)"
echo ""
echo "=== Services ==="
systemctl is-active docker &>/dev/null && echo "Docker: Running" || echo "Docker: Stopped"
systemctl is-active code-tunnel &>/dev/null && echo "VS Code Tunnel: Running" || echo "VS Code Tunnel: Stopped"
tailscale status &>/dev/null && echo "Tailscale: Connected" || echo "Tailscale: Disconnected"
echo ""
echo "=== Resources ==="
echo "CPU: $(nproc) cores"
echo "RAM: $(free -h | awk '/^Mem:/ {print $2}')"
echo "Disk: $(df -h / | awk 'NR==2 {print $4 " free"}')"
EOF
    chmod +x /usr/local/bin/devbox-status

    # Quick connect script
    cat > /usr/local/bin/devbox-connect << 'EOF'
#!/bin/bash
echo "=== Connection Options ==="
echo ""
TS_IP=$(tailscale ip -4 2>/dev/null)
if [ -n "$TS_IP" ]; then
    echo "SSH (Tailscale): ssh devbox@$TS_IP"
fi
EXT_IP=$(curl -s ifconfig.me)
echo "SSH (External):  ssh devbox@$EXT_IP"
echo ""
echo "VS Code Tunnel:  code-tunnel tunnel"
echo "Start service:   sudo systemctl start code-tunnel"
EOF
    chmod +x /usr/local/bin/devbox-connect
}

# ============================================================================
# Main
# ============================================================================
main() {
    setup_user
    install_base
    install_node
    install_docker
    install_rust
    install_go
    install_tailscale
    setup_repo
    setup_vscode_tunnel
    create_helper_scripts

    echo ""
    echo "=========================================="
    echo "DevBox Setup Complete!"
    echo "=========================================="
    echo ""
    devbox-status
    echo ""
    devbox-connect
}

main "$@"
