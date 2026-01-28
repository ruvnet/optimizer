# RuVector Memory Optimizer - Deploy to Mac Mini
# Syncs the project to Mac Mini via Tailscale and runs tests
#
# Usage: .\scripts\deploy-to-mac.ps1

param(
    [string]$MacHost = "100.123.117.38",
    [string]$MacUser = "cohen",
    [string]$RemotePath = "~/workspace/ruvector-memopt",
    [switch]$BuildOnly,
    [switch]$TestOnly,
    [switch]$Install
)

$ErrorActionPreference = "Stop"

Write-Host "`n═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  RuVector Memory Optimizer - Mac Mini Deployment" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════`n" -ForegroundColor Cyan

Write-Host "Target: ${MacUser}@${MacHost}:${RemotePath}" -ForegroundColor Yellow

# Check SSH connectivity
Write-Host "`nChecking SSH connectivity..." -ForegroundColor Blue
$sshTest = ssh -o ConnectTimeout=5 "${MacUser}@${MacHost}" "echo 'connected'" 2>&1
if ($sshTest -ne "connected") {
    Write-Host "ERROR: Cannot connect to Mac Mini via SSH" -ForegroundColor Red
    Write-Host "Make sure Tailscale is running and connected." -ForegroundColor Yellow
    exit 1
}
Write-Host "SSH connection OK" -ForegroundColor Green

# Create remote directory if needed
Write-Host "`nCreating remote directory..." -ForegroundColor Blue
ssh "${MacUser}@${MacHost}" "mkdir -p ${RemotePath}"

if (-not $TestOnly) {
    # Sync files using rsync over SSH
    Write-Host "`nSyncing project files..." -ForegroundColor Blue

    # Use rsync if available, otherwise scp
    $rsyncAvailable = Get-Command rsync -ErrorAction SilentlyContinue

    if ($rsyncAvailable) {
        # Exclude unnecessary files
        rsync -avz --progress `
            --exclude 'target/' `
            --exclude '.git/' `
            --exclude 'dist/' `
            --exclude 'installer/' `
            --exclude '*.exe' `
            --exclude '*.msi' `
            --exclude 'node_modules/' `
            . "${MacUser}@${MacHost}:${RemotePath}/"
    } else {
        Write-Host "rsync not found, using scp (slower)..." -ForegroundColor Yellow

        # Create a list of files to copy (excluding large directories)
        $filesToCopy = @(
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "src",
            "scripts",
            "docs"
        )

        foreach ($item in $filesToCopy) {
            if (Test-Path $item) {
                Write-Host "  Copying $item..." -ForegroundColor Gray
                scp -r $item "${MacUser}@${MacHost}:${RemotePath}/"
            }
        }
    }

    Write-Host "Sync complete" -ForegroundColor Green
}

if (-not $BuildOnly) {
    # Run test script on Mac
    Write-Host "`n═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
    Write-Host "  Running Tests on Mac Mini" -ForegroundColor Cyan
    Write-Host "═══════════════════════════════════════════════════════════`n" -ForegroundColor Cyan

    # Make test script executable and run it
    ssh "${MacUser}@${MacHost}" @"
cd ${RemotePath}
chmod +x scripts/test-mac.sh
./scripts/test-mac.sh
"@
}

if ($Install) {
    Write-Host "`nInstalling to /usr/local/bin on Mac..." -ForegroundColor Blue
    ssh "${MacUser}@${MacHost}" "sudo cp ${RemotePath}/target/release/ruvector-memopt /usr/local/bin/"
    Write-Host "Installed!" -ForegroundColor Green
}

Write-Host "`n═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  Deployment Complete!" -ForegroundColor Green
Write-Host "═══════════════════════════════════════════════════════════`n" -ForegroundColor Cyan

Write-Host "To connect and use manually:"
Write-Host "  ssh ${MacUser}@${MacHost}"
Write-Host "  cd ${RemotePath}"
Write-Host "  ./target/release/ruvector-memopt status" -ForegroundColor Yellow
Write-Host ""
