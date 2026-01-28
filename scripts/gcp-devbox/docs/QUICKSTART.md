# DevBox Quick Reference

## Setup (One-time)

```bash
# 1. Install gcloud
winget install Google.CloudSDK   # Windows
brew install google-cloud-sdk    # Mac

# 2. Login
gcloud auth login
gcloud config set project YOUR_PROJECT

# 3. Optional: Tailscale auto-connect
$env:TAILSCALE_AUTHKEY = "tskey-auth-xxxxx"   # PowerShell
export TAILSCALE_AUTHKEY="tskey-auth-xxxxx"   # Bash
```

## Daily Usage

```bash
# Create new DevBox (interactive)
.\devbox.ps1 create -i

# Check running instances
.\devbox.ps1 status

# Connect to DevBox
.\devbox.ps1 connect <name>

# Stop when done (saves money!)
.\devbox.ps1 stop <name>

# Resume next day
.\devbox.ps1 start <name>
```

## Common Scenarios

### Start Fresh with claude-flow
```bash
.\devbox.ps1 create -r claude-flow
```

### ML Development with GPU
```bash
.\devbox.ps1 create -r ml-workspace -g nvidia-tesla-t4
```

### Custom Repo
```bash
.\devbox.ps1 create -i
# Select "custom" and enter URL
```

### VS Code Remote
```bash
code --remote ssh-remote+devbox@100.x.x.x /home/devbox/workspace
```

## Costs

| Action | Cost |
|--------|------|
| Running (e2-standard-4) | ~$0.13/hour |
| Stopped | ~$0.003/hour (storage only) |
| Delete | $0 |

**Tip**: Always `stop` when not using!

## On the DevBox

```bash
devbox-status    # Show IPs and service status
devbox-connect   # Show connection options
```

## Troubleshooting

```bash
# View setup logs
ssh devbox@IP "sudo cat /var/log/devbox-setup.log"

# Manually connect Tailscale
ssh devbox@IP "sudo tailscale up"

# Re-run setup
ssh devbox@IP "sudo /var/lib/google/startup-scripts/startup"
```
