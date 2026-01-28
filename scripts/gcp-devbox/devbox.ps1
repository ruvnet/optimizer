# GCP DevBox CLI - PowerShell Version
# Codespace-style Development Environments for Windows

param(
    [Parameter(Position=0)]
    [string]$Command = "help",

    [Alias("r")]
    [string]$Repo,

    [Alias("m")]
    [string]$Machine,

    [Alias("z")]
    [string]$Zone = "us-central1-a",

    [Alias("p")]
    [string]$Project,

    [Alias("g")]
    [string]$Gpu,

    [Alias("t")]
    [string]$TailscaleKey,

    [Alias("d")]
    [string]$DiskSize,

    [Alias("i")]
    [switch]$Interactive,

    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$RemainingArgs
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ConfigFile = Join-Path $ScriptDir "repos.json"
$SetupScript = Join-Path $ScriptDir "setup-vm.sh"

# ============================================================================
# Helper Functions
# ============================================================================
function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[OK] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }
function Write-Error { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Test-GCloud {
    if (-not (Get-Command gcloud -ErrorAction SilentlyContinue)) {
        Write-Error "gcloud CLI not found. Install from https://cloud.google.com/sdk/docs/install"
        exit 1
    }
}

function Get-Config {
    param($Key)
    $config = Get-Content $ConfigFile | ConvertFrom-Json
    $parts = $Key -split '\.'
    $value = $config
    foreach ($part in $parts) {
        $value = $value.$part
    }
    return $value
}

function Get-RepoConfig {
    param($RepoName)
    $config = Get-Content $ConfigFile | ConvertFrom-Json
    return $config.repos.$RepoName
}

# ============================================================================
# List available repos
# ============================================================================
function Show-RepoList {
    $config = Get-Content $ConfigFile | ConvertFrom-Json

    Write-Host ""
    Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║              Available Development Environments                ║" -ForegroundColor Cyan
    Write-Host "╠════════════════════════════════════════════════════════════════╣" -ForegroundColor Cyan

    $repos = $config.repos.PSObject.Properties | Select-Object -ExpandProperty Name
    $i = 1

    foreach ($repo in $repos) {
        $repoConfig = $config.repos.$repo
        $desc = $repoConfig.description
        $machine = $repoConfig.machine_type
        $gpu = if ($repoConfig.gpu) { $repoConfig.gpu } else { "none" }

        Write-Host ("║ " + "{0,2}" -f $i + ". " + "{0,-15}" -f $repo + " " + "{0,-30}" -f $desc + "║") -ForegroundColor Cyan
        Write-Host ("║     Machine: " + "{0,-12}" -f $machine + " GPU: " + "{0,-20}" -f $gpu + "║") -ForegroundColor Cyan
        $i++
    }

    Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""

    return $repos
}

# ============================================================================
# Interactive repo selection
# ============================================================================
function Select-Repo {
    $repos = Show-RepoList
    $repoArray = @($repos)
    $count = $repoArray.Count

    $selection = Read-Host "Select environment (1-$count)"
    $index = [int]$selection - 1

    if ($index -lt 0 -or $index -ge $count) {
        Write-Error "Invalid selection"
        exit 1
    }

    $selectedRepo = $repoArray[$index]

    if ($selectedRepo -eq "custom") {
        $customUrl = Read-Host "Enter custom repository URL"
        return @{ Name = "custom"; Url = $customUrl }
    }

    $config = Get-Content $ConfigFile | ConvertFrom-Json
    return @{ Name = $selectedRepo; Url = $config.repos.$selectedRepo.url }
}

# ============================================================================
# Create DevBox
# ============================================================================
function New-DevBox {
    Test-GCloud

    $repoName = $Repo
    $repoUrl = ""

    # Interactive mode
    if ($Interactive -or -not $repoName) {
        $selection = Select-Repo
        $repoName = $selection.Name
        $repoUrl = $selection.Url
    }

    # Get repo config
    $repoConfig = Get-RepoConfig $repoName
    if (-not $repoUrl) { $repoUrl = $repoConfig.url }
    if (-not $Machine) { $Machine = $repoConfig.machine_type }
    if (-not $DiskSize) { $DiskSize = $repoConfig.disk_size }
    if (-not $Gpu) { $Gpu = $repoConfig.gpu }
    $postClone = ($repoConfig.post_clone -join "`n")

    # Get project
    if (-not $Project) {
        $Project = gcloud config get-value project 2>$null
    }

    # Get defaults
    $imageFamily = Get-Config "defaults.image_family"
    $imageProject = Get-Config "defaults.image_project"

    # Tailscale key from env
    if (-not $TailscaleKey) {
        $TailscaleKey = $env:TAILSCALE_AUTHKEY
    }

    # Generate instance name
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $instanceName = "devbox-$repoName-$timestamp"

    Write-Host ""
    Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║                    Creating DevBox Instance                    ║" -ForegroundColor Cyan
    Write-Host "╠════════════════════════════════════════════════════════════════╣" -ForegroundColor Cyan
    Write-Host "║ Instance:  $instanceName" -ForegroundColor Cyan
    Write-Host "║ Project:   $Project" -ForegroundColor Yellow
    Write-Host "║ Zone:      $Zone" -ForegroundColor Yellow
    Write-Host "║ Machine:   $Machine" -ForegroundColor Yellow
    Write-Host "║ Disk:      ${DiskSize}GB" -ForegroundColor Yellow
    Write-Host "║ GPU:       $(if ($Gpu) { $Gpu } else { 'none' })" -ForegroundColor Yellow
    Write-Host "║ Repo:      $repoUrl" -ForegroundColor Yellow
    Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""

    $confirm = Read-Host "Create this DevBox? (y/N)"
    if ($confirm -notmatch "^[Yy]$") {
        Write-Warn "Cancelled"
        return
    }

    # Build metadata
    $metadata = @(
        "REPO_URL=$repoUrl",
        "REPO_NAME=$repoName",
        "POST_CLONE_COMMANDS=$postClone",
        "GITHUB_TOKEN=$env:GITHUB_TOKEN",
        "TAILSCALE_AUTHKEY=$TailscaleKey"
    ) -join ","

    # Build gcloud command
    $gcloudArgs = @(
        "compute", "instances", "create", $instanceName,
        "--project=$Project",
        "--zone=$Zone",
        "--machine-type=$Machine",
        "--image-family=$imageFamily",
        "--image-project=$imageProject",
        "--boot-disk-size=${DiskSize}GB",
        "--boot-disk-type=pd-ssd",
        "--tags=devbox,http-server,https-server",
        "--metadata-from-file=startup-script=$SetupScript",
        "--metadata=$metadata",
        "--scopes=cloud-platform"
    )

    # Add GPU if specified
    if ($Gpu -and $Gpu -ne "null") {
        $gcloudArgs += "--accelerator=type=$Gpu,count=1"
        $gcloudArgs += "--maintenance-policy=TERMINATE"
    }

    Write-Info "Creating instance..."
    & gcloud @gcloudArgs

    Write-Success "Instance created: $instanceName"

    Write-Info "Waiting for setup to complete (this may take 3-5 minutes)..."
    Start-Sleep -Seconds 30

    # Get external IP
    $externalIp = gcloud compute instances describe $instanceName `
        --zone=$Zone `
        --format="get(networkInterfaces[0].accessConfigs[0].natIP)"

    Write-Host ""
    Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Green
    Write-Host "║                     DevBox Ready!                              ║" -ForegroundColor Green
    Write-Host "╠════════════════════════════════════════════════════════════════╣" -ForegroundColor Green
    Write-Host "║ SSH:       gcloud compute ssh $instanceName --zone=$Zone" -ForegroundColor Cyan
    Write-Host "║ Direct:    ssh devbox@$externalIp" -ForegroundColor Cyan
    Write-Host "║" -ForegroundColor Green
    Write-Host "║ VS Code:   code --remote ssh-remote+devbox@$externalIp /home/devbox/workspace" -ForegroundColor Cyan
    Write-Host "║" -ForegroundColor Green
    Write-Host "║ Tailscale: Run 'devbox-status' on VM for Tailscale IP" -ForegroundColor Yellow
    Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Green
}

# ============================================================================
# Status
# ============================================================================
function Get-DevBoxStatus {
    Test-GCloud

    Write-Host ""
    Write-Host "Running DevBox Instances:" -ForegroundColor Cyan
    Write-Host ""

    gcloud compute instances list --filter="tags.items=devbox" `
        --format="table(name,zone,machineType.basename(),status,networkInterfaces[0].accessConfigs[0].natIP:label=EXTERNAL_IP)"
}

# ============================================================================
# Connect
# ============================================================================
function Connect-DevBox {
    Test-GCloud

    $instanceName = $RemainingArgs[0]
    $connectZone = if ($RemainingArgs[1]) { $RemainingArgs[1] } else { $Zone }

    if (-not $instanceName) {
        Write-Host "Select a DevBox to connect:"
        gcloud compute instances list --filter="tags.items=devbox AND status=RUNNING" `
            --format="table[no-heading](name,zone)"

        $instanceName = Read-Host "Instance name"
        $connectZone = Read-Host "Zone"
    }

    Write-Info "Connecting to $instanceName..."
    gcloud compute ssh $instanceName --zone=$connectZone -- -A
}

# ============================================================================
# Stop
# ============================================================================
function Stop-DevBox {
    $instanceName = $RemainingArgs[0]
    $stopZone = if ($RemainingArgs[1]) { $RemainingArgs[1] } else { $Zone }

    if (-not $instanceName) {
        Write-Error "Usage: devbox stop <instance-name> [zone]"
        return
    }

    Write-Info "Stopping $instanceName..."
    gcloud compute instances stop $instanceName --zone=$stopZone
    Write-Success "Instance stopped"
}

# ============================================================================
# Start
# ============================================================================
function Start-DevBox {
    $instanceName = $RemainingArgs[0]
    $startZone = if ($RemainingArgs[1]) { $RemainingArgs[1] } else { $Zone }

    if (-not $instanceName) {
        Write-Error "Usage: devbox start <instance-name> [zone]"
        return
    }

    Write-Info "Starting $instanceName..."
    gcloud compute instances start $instanceName --zone=$startZone
    Write-Success "Instance started"
}

# ============================================================================
# Delete
# ============================================================================
function Remove-DevBox {
    $instanceName = $RemainingArgs[0]
    $deleteZone = if ($RemainingArgs[1]) { $RemainingArgs[1] } else { $Zone }

    if (-not $instanceName) {
        Write-Error "Usage: devbox delete <instance-name> [zone]"
        return
    }

    $confirm = Read-Host "Delete $instanceName? This cannot be undone. (y/N)"
    if ($confirm -notmatch "^[Yy]$") {
        Write-Warn "Cancelled"
        return
    }

    Write-Info "Deleting $instanceName..."
    gcloud compute instances delete $instanceName --zone=$deleteZone --quiet
    Write-Success "Instance deleted"
}

# ============================================================================
# SSH Config
# ============================================================================
function Get-SshConfig {
    Test-GCloud

    Write-Host ""
    Write-Host "# Add to ~/.ssh/config"
    Write-Host ""

    $instances = gcloud compute instances list --filter="tags.items=devbox AND status=RUNNING" `
        --format="csv[no-heading](name,zone,networkInterfaces[0].accessConfigs[0].natIP)"

    foreach ($line in $instances -split "`n") {
        if ($line) {
            $parts = $line -split ","
            $name = $parts[0]
            $instZone = $parts[1]
            $ip = $parts[2]

            Write-Host "Host $name"
            Write-Host "    HostName $ip"
            Write-Host "    User devbox"
            Write-Host "    ForwardAgent yes"
            Write-Host ""
        }
    }
}

# ============================================================================
# Help
# ============================================================================
function Show-Help {
    Write-Host ""
    Write-Host "GCP DevBox - Codespace-style Development Environments" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Usage: .\devbox.ps1 <command> [options]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  list              List available repo templates"
    Write-Host "  create            Create a new DevBox instance"
    Write-Host "  status            Show running DevBox instances"
    Write-Host "  connect <name>    SSH into a DevBox"
    Write-Host "  start <name>      Start a stopped DevBox"
    Write-Host "  stop <name>       Stop a running DevBox"
    Write-Host "  delete <name>     Delete a DevBox"
    Write-Host "  ssh-config        Generate SSH config for all DevBoxes"
    Write-Host ""
    Write-Host "Create Options:"
    Write-Host "  -r, -Repo <name>         Repository template name"
    Write-Host "  -m, -Machine <type>      Machine type (e.g., e2-standard-4)"
    Write-Host "  -z, -Zone <zone>         GCP zone (default: us-central1-a)"
    Write-Host "  -p, -Project <id>        GCP project ID"
    Write-Host "  -g, -Gpu <type>          GPU type (e.g., nvidia-tesla-t4)"
    Write-Host "  -t, -TailscaleKey        Tailscale auth key"
    Write-Host "  -d, -DiskSize <size>     Disk size in GB"
    Write-Host "  -i, -Interactive         Interactive repo selection"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\devbox.ps1 create -i                    # Interactive mode"
    Write-Host "  .\devbox.ps1 create -r claude-flow        # Create claude-flow DevBox"
    Write-Host "  .\devbox.ps1 create -r ml-workspace -g nvidia-tesla-t4"
    Write-Host "  .\devbox.ps1 connect devbox-claude-flow-20240115"
    Write-Host ""
    Write-Host "Environment Variables:"
    Write-Host "  TAILSCALE_AUTHKEY    Tailscale auth key for auto-connect"
    Write-Host "  GITHUB_TOKEN         GitHub token for private repos"
    Write-Host ""
}

# ============================================================================
# Main
# ============================================================================
switch ($Command.ToLower()) {
    "list" { Show-RepoList | Out-Null }
    "create" { New-DevBox }
    "status" { Get-DevBoxStatus }
    "connect" { Connect-DevBox }
    "start" { Start-DevBox }
    "stop" { Stop-DevBox }
    "delete" { Remove-DevBox }
    "ssh-config" { Get-SshConfig }
    "help" { Show-Help }
    default { Write-Error "Unknown command: $Command"; Show-Help }
}
