# GCP DevBox

A Codespace/DevPod-style development environment manager for Google Cloud Platform.

Spin up pre-configured development VMs with your GitHub repos, Tailscale networking, and claude-flow pre-installed in seconds.

## Features

- **One-command VM creation** with your repos pre-cloned
- **Tailscale integration** for secure access without firewall rules
- **VS Code tunnel support** for browser-based editing
- **Auto-installs** Node.js, Docker, Rust, Go, and dev tools
- **GPU support** for ML workloads
- **Cost-optimized** with easy stop/start commands

## Quick Start

### Prerequisites

1. **Google Cloud SDK** installed and authenticated:
   ```bash
   # Windows
   winget install Google.CloudSDK

   # Mac
   brew install google-cloud-sdk

   # Linux
   curl https://sdk.cloud.google.com | bash
   ```

2. **Authenticate and set project**:
   ```bash
   gcloud auth login
   gcloud config set project YOUR_PROJECT_ID
   ```

3. **(Optional) Tailscale auth key** for automatic VPN setup:
   - Get key from: https://login.tailscale.com/admin/settings/keys
   - Create a reusable, ephemeral key
   ```bash
   export TAILSCALE_AUTHKEY="tskey-auth-xxxxx"
   ```

4. **(Optional) GitHub token** for private repos:
   ```bash
   export GITHUB_TOKEN="ghp_xxxxx"
   ```

### Create Your First DevBox

```bash
# Windows PowerShell
.\devbox.ps1 create -i

# Linux/Mac
./devbox.sh create -i
```

This will:
1. Show available repo templates
2. Let you select one
3. Create a GCP VM with all tools installed
4. Clone your repo and run setup commands
5. Connect to Tailscale (if key provided)
6. Display connection instructions

## Commands Reference

### List Available Templates

```bash
.\devbox.ps1 list
```

Shows all pre-configured repo templates with machine specs.

### Create DevBox

```bash
# Interactive mode (recommended)
.\devbox.ps1 create -i

# Specify repo directly
.\devbox.ps1 create -r claude-flow

# With custom options
.\devbox.ps1 create -r claude-flow -m e2-standard-8 -d 100

# With GPU
.\devbox.ps1 create -r ml-workspace -g nvidia-tesla-t4
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-r, --repo` | Repository template name | (interactive) |
| `-m, --machine` | Machine type | From template |
| `-z, --zone` | GCP zone | us-central1-a |
| `-p, --project` | GCP project ID | Current project |
| `-g, --gpu` | GPU type | None |
| `-t, --tailscale-key` | Tailscale auth key | $TAILSCALE_AUTHKEY |
| `-d, --disk` | Disk size in GB | From template |
| `-i, --interactive` | Interactive selection | false |

### Check Status

```bash
.\devbox.ps1 status
```

Lists all running DevBox instances with IPs.

### Connect via SSH

```bash
# By name
.\devbox.ps1 connect devbox-claude-flow-20240115

# Interactive selection
.\devbox.ps1 connect
```

### Stop Instance (Save Costs)

```bash
.\devbox.ps1 stop devbox-claude-flow-20240115
```

Stops the VM but preserves disk. You only pay for storage (~$0.10/GB/month).

### Start Instance

```bash
.\devbox.ps1 start devbox-claude-flow-20240115
```

Resumes a stopped instance. Takes ~30 seconds.

### Delete Instance

```bash
.\devbox.ps1 delete devbox-claude-flow-20240115
```

Permanently deletes the VM and disk. Cannot be undone.

### Generate SSH Config

```bash
.\devbox.ps1 ssh-config >> ~/.ssh/config
```

Generates SSH config entries for all running DevBoxes.

## Connecting to Your DevBox

### Option 1: Tailscale (Recommended)

If you provided a Tailscale auth key, your DevBox auto-connects to your tailnet.

```bash
# Find Tailscale IP (run on DevBox)
devbox-status

# SSH from anywhere
ssh devbox@100.x.x.x
```

### Option 2: gcloud SSH

```bash
gcloud compute ssh devbox-claude-flow-20240115 --zone=us-central1-a
```

Works through IAP tunnel, no external IP needed.

### Option 3: Direct SSH

```bash
ssh devbox@EXTERNAL_IP
```

Requires firewall rule for port 22 (created by default).

### Option 4: VS Code

```bash
# Remote SSH extension
code --remote ssh-remote+devbox@IP /home/devbox/workspace

# Or use VS Code tunnel (browser-based)
# On DevBox:
sudo systemctl start code-tunnel
# Then connect via VS Code: Remote Tunnels > Connect
```

## Pre-configured Templates

| Template | Machine | Disk | GPU | Post-Clone |
|----------|---------|------|-----|------------|
| claude-flow | e2-standard-4 | 50GB | - | npm install, claude-flow init |
| agentic-flow | e2-standard-4 | 50GB | - | npm install |
| agentdb | e2-standard-4 | 50GB | - | npm install |
| bot-generator | e2-standard-4 | 50GB | - | npm install |
| federated-mcp | e2-standard-4 | 50GB | - | npm install |
| ruvector-memopt | e2-standard-2 | 30GB | - | - |
| ml-workspace | n1-standard-8 | 100GB | T4 | pip install -r requirements.txt |
| custom | e2-standard-4 | 50GB | - | - |

## Adding Custom Templates

Edit `repos.json`:

```json
{
  "repos": {
    "my-project": {
      "url": "https://github.com/username/my-project",
      "description": "My custom project",
      "machine_type": "e2-standard-8",
      "disk_size": "100",
      "gpu": null,
      "post_clone": [
        "npm install",
        "npm run build",
        "npx @claude-flow/cli@latest init --force"
      ]
    }
  }
}
```

### Machine Types

| Type | vCPUs | RAM | Use Case | $/hour |
|------|-------|-----|----------|--------|
| e2-standard-2 | 2 | 8GB | Light dev | ~$0.07 |
| e2-standard-4 | 4 | 16GB | General dev | ~$0.13 |
| e2-standard-8 | 8 | 32GB | Heavy dev | ~$0.27 |
| n1-standard-8 | 8 | 30GB | GPU workloads | ~$0.38 |
| n2-standard-16 | 16 | 64GB | Large projects | ~$0.78 |
| c3-standard-8 | 8 | 32GB | AVX-512 support | ~$0.40 |

### GPU Types

| GPU | VRAM | $/hour | Use Case |
|-----|------|--------|----------|
| nvidia-tesla-t4 | 16GB | ~$0.35 | ML inference |
| nvidia-l4 | 24GB | ~$0.70 | ML training |
| nvidia-tesla-v100 | 16GB | ~$2.48 | Heavy ML |
| nvidia-a100-40gb | 40GB | ~$3.67 | Large models |

## What Gets Installed

Every DevBox comes with:

### Base Tools
- Git, curl, wget, jq, htop, tmux
- Build essentials (gcc, make, etc.)

### Languages
- **Node.js 20** with npm, pnpm, yarn
- **Python 3.11** with pip, venv
- **Rust** (latest stable)
- **Go 1.22**

### Dev Tools
- **Docker** with docker-compose
- **Claude Code** (`@anthropic-ai/claude-code`)
- **Claude Flow** (`@claude-flow/cli`)

### Networking
- **Tailscale** for secure VPN access
- **VS Code Server** for tunnel connections

## Helper Commands (On DevBox)

After SSH'ing into your DevBox:

```bash
# Show status and connection info
devbox-status

# Show all connection options
devbox-connect

# Start VS Code tunnel
sudo systemctl start code-tunnel
```

## Cost Management

### Estimated Costs

| Usage | e2-standard-4 | With T4 GPU |
|-------|---------------|-------------|
| 8 hours/day | ~$32/month | ~$116/month |
| 24/7 | ~$95/month | ~$345/month |
| Stopped (storage only) | ~$5/month | ~$10/month |

### Tips to Save Money

1. **Stop when not using**: `.\devbox.ps1 stop <name>`
2. **Use Spot VMs** for non-critical work (add to setup-vm.sh)
3. **Right-size machines**: Start small, scale up if needed
4. **Delete unused instances**: `.\devbox.ps1 delete <name>`
5. **Use scheduling**: Set up auto-stop during off hours

### Auto-Stop Script

Add to VM cron for automatic shutdown after idle:

```bash
# On DevBox, add to crontab
*/30 * * * * pgrep -u devbox ssh || sudo shutdown -h +5
```

## Troubleshooting

### "Permission denied" on SSH

```bash
# Ensure your SSH key is added
gcloud compute config-ssh
```

### Tailscale not connecting

```bash
# On DevBox, manually connect
sudo tailscale up
```

### VM creation fails

```bash
# Check quota
gcloud compute regions describe us-central1

# Try different zone
.\devbox.ps1 create -r claude-flow -z us-west1-a
```

### Setup script didn't complete

```bash
# Check logs on VM
sudo cat /var/log/devbox-setup.log

# Re-run setup
sudo /var/lib/google/startup-scripts/startup
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Your Workstation                        │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  devbox.ps1 / devbox.sh                             │   │
│  │  - Manages GCP instances via gcloud CLI             │   │
│  │  - Reads repos.json for templates                   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ gcloud compute instances create
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Google Cloud Platform                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  DevBox VM (Ubuntu 24.04)                           │   │
│  │  ┌─────────────────────────────────────────────┐   │   │
│  │  │  setup-vm.sh (startup script)               │   │   │
│  │  │  - Installs Node, Docker, Rust, Go          │   │   │
│  │  │  - Installs Tailscale, VS Code CLI          │   │   │
│  │  │  - Clones repo, runs post-clone commands    │   │   │
│  │  │  - Initializes claude-flow                  │   │   │
│  │  └─────────────────────────────────────────────┘   │   │
│  │                                                     │   │
│  │  /home/devbox/workspace/<repo>                     │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ Tailscale VPN
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Your Tailnet                           │
│  - Secure access from anywhere                              │
│  - No port forwarding needed                                │
│  - SSH via Tailscale IP: ssh devbox@100.x.x.x              │
└─────────────────────────────────────────────────────────────┘
```

## Files

```
scripts/gcp-devbox/
├── devbox.sh          # Bash CLI (Linux/Mac)
├── devbox.ps1         # PowerShell CLI (Windows)
├── setup-vm.sh        # VM startup script
├── repos.json         # Repository templates
└── docs/
    └── README.md      # This file
```

## Contributing

To add features or fix bugs:

1. Edit `setup-vm.sh` for VM configuration changes
2. Edit `devbox.sh` / `devbox.ps1` for CLI changes
3. Edit `repos.json` for new templates

## License

MIT
