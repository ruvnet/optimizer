#!/bin/bash
# GCP DevBox CLI - Codespace-style Development Environments
# Usage: ./devbox.sh [command] [options]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$SCRIPT_DIR/repos.json"
SETUP_SCRIPT="$SCRIPT_DIR/setup-vm.sh"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_gcloud() {
    if ! command -v gcloud &> /dev/null; then
        log_error "gcloud CLI not found. Install from https://cloud.google.com/sdk/docs/install"
        exit 1
    fi
}

check_jq() {
    if ! command -v jq &> /dev/null; then
        log_error "jq not found. Install with: apt install jq / brew install jq"
        exit 1
    fi
}

get_config() {
    local key="$1"
    jq -r "$key" "$CONFIG_FILE"
}

# ============================================================================
# List available repos
# ============================================================================
cmd_list() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║              Available Development Environments                ║${NC}"
    echo -e "${CYAN}╠════════════════════════════════════════════════════════════════╣${NC}"

    local repos=$(jq -r '.repos | keys[]' "$CONFIG_FILE")
    local i=1

    for repo in $repos; do
        local desc=$(jq -r ".repos[\"$repo\"].description" "$CONFIG_FILE")
        local machine=$(jq -r ".repos[\"$repo\"].machine_type" "$CONFIG_FILE")
        local gpu=$(jq -r ".repos[\"$repo\"].gpu // \"none\"" "$CONFIG_FILE")

        printf "${CYAN}║${NC} ${GREEN}%2d${NC}. %-15s ${YELLOW}%-30s${NC}${CYAN}║${NC}\n" "$i" "$repo" "$desc"
        printf "${CYAN}║${NC}     Machine: %-12s GPU: %-20s${CYAN}║${NC}\n" "$machine" "$gpu"
        ((i++))
    done

    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

# ============================================================================
# Interactive repo selection
# ============================================================================
select_repo() {
    cmd_list

    local repos=($(jq -r '.repos | keys[]' "$CONFIG_FILE"))
    local count=${#repos[@]}

    echo -n "Select environment (1-$count): "
    read -r selection

    if [[ "$selection" -lt 1 || "$selection" -gt "$count" ]]; then
        log_error "Invalid selection"
        exit 1
    fi

    SELECTED_REPO="${repos[$((selection-1))]}"

    if [ "$SELECTED_REPO" == "custom" ]; then
        echo -n "Enter custom repository URL: "
        read -r CUSTOM_URL
        REPO_URL="$CUSTOM_URL"
    else
        REPO_URL=$(jq -r ".repos[\"$SELECTED_REPO\"].url" "$CONFIG_FILE")
    fi

    log_info "Selected: $SELECTED_REPO"
}

# ============================================================================
# Create DevBox
# ============================================================================
cmd_create() {
    check_gcloud
    check_jq

    local repo_name=""
    local machine_type=""
    local zone=""
    local project=""
    local gpu=""
    local tailscale_key=""
    local disk_size=""

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -r|--repo) repo_name="$2"; shift 2 ;;
            -m|--machine) machine_type="$2"; shift 2 ;;
            -z|--zone) zone="$2"; shift 2 ;;
            -p|--project) project="$2"; shift 2 ;;
            -g|--gpu) gpu="$2"; shift 2 ;;
            -t|--tailscale-key) tailscale_key="$2"; shift 2 ;;
            -d|--disk) disk_size="$2"; shift 2 ;;
            -i|--interactive) select_repo; repo_name="$SELECTED_REPO"; shift ;;
            *) log_error "Unknown option: $1"; exit 1 ;;
        esac
    done

    # Interactive mode if no repo specified
    if [ -z "$repo_name" ]; then
        select_repo
        repo_name="$SELECTED_REPO"
    fi

    # Get repo config
    local repo_url=$(jq -r ".repos[\"$repo_name\"].url // \"\"" "$CONFIG_FILE")
    machine_type="${machine_type:-$(jq -r ".repos[\"$repo_name\"].machine_type" "$CONFIG_FILE")}"
    disk_size="${disk_size:-$(jq -r ".repos[\"$repo_name\"].disk_size" "$CONFIG_FILE")}"
    gpu="${gpu:-$(jq -r ".repos[\"$repo_name\"].gpu // \"\"" "$CONFIG_FILE")}"
    local post_clone=$(jq -r ".repos[\"$repo_name\"].post_clone | join(\"\n\")" "$CONFIG_FILE")

    # Defaults
    zone="${zone:-$(get_config '.defaults.zone')}"
    project="${project:-$(gcloud config get-value project 2>/dev/null)}"
    local image_family=$(get_config '.defaults.image_family')
    local image_project=$(get_config '.defaults.image_project')

    # Tailscale auth key from env if not provided
    if [ -z "$tailscale_key" ]; then
        tailscale_key="${TAILSCALE_AUTHKEY:-}"
    fi

    # Generate instance name
    local instance_name="devbox-${repo_name}-$(date +%Y%m%d-%H%M%S)"

    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                    Creating DevBox Instance                    ║${NC}"
    echo -e "${CYAN}╠════════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${CYAN}║${NC} Instance:  ${GREEN}$instance_name${NC}"
    echo -e "${CYAN}║${NC} Project:   ${YELLOW}$project${NC}"
    echo -e "${CYAN}║${NC} Zone:      ${YELLOW}$zone${NC}"
    echo -e "${CYAN}║${NC} Machine:   ${YELLOW}$machine_type${NC}"
    echo -e "${CYAN}║${NC} Disk:      ${YELLOW}${disk_size}GB${NC}"
    echo -e "${CYAN}║${NC} GPU:       ${YELLOW}${gpu:-none}${NC}"
    echo -e "${CYAN}║${NC} Repo:      ${YELLOW}$repo_url${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    # Confirm
    echo -n "Create this DevBox? (y/N): "
    read -r confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        log_warn "Cancelled"
        exit 0
    fi

    # Build gcloud command
    local gcloud_cmd="gcloud compute instances create $instance_name \
        --project=$project \
        --zone=$zone \
        --machine-type=$machine_type \
        --image-family=$image_family \
        --image-project=$image_project \
        --boot-disk-size=${disk_size}GB \
        --boot-disk-type=pd-ssd \
        --tags=devbox,http-server,https-server \
        --metadata-from-file=startup-script=$SETUP_SCRIPT"

    # Add metadata
    local metadata="REPO_URL=$repo_url"
    metadata+=",REPO_NAME=$repo_name"
    metadata+=",POST_CLONE_COMMANDS=$post_clone"
    metadata+=",GITHUB_TOKEN=${GITHUB_TOKEN:-}"
    metadata+=",TAILSCALE_AUTHKEY=$tailscale_key"

    gcloud_cmd+=" --metadata=\"$metadata\""

    # Add GPU if specified
    if [ -n "$gpu" ] && [ "$gpu" != "null" ]; then
        gcloud_cmd+=" --accelerator=type=$gpu,count=1 --maintenance-policy=TERMINATE"
    fi

    # Add scopes for GCP APIs
    gcloud_cmd+=" --scopes=cloud-platform"

    log_info "Creating instance..."
    eval "$gcloud_cmd"

    log_success "Instance created: $instance_name"

    # Wait for startup script
    log_info "Waiting for setup to complete (this may take 3-5 minutes)..."
    sleep 30

    # Get connection info
    local external_ip=$(gcloud compute instances describe "$instance_name" \
        --zone="$zone" \
        --format='get(networkInterfaces[0].accessConfigs[0].natIP)')

    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                     DevBox Ready!                              ║${NC}"
    echo -e "${GREEN}╠════════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${GREEN}║${NC} SSH:       ${CYAN}gcloud compute ssh $instance_name --zone=$zone${NC}"
    echo -e "${GREEN}║${NC} Direct:    ${CYAN}ssh devbox@$external_ip${NC}"
    echo -e "${GREEN}║${NC}"
    echo -e "${GREEN}║${NC} VS Code:   ${CYAN}code --remote ssh-remote+devbox@$external_ip /home/devbox/workspace${NC}"
    echo -e "${GREEN}║${NC}"
    echo -e "${GREEN}║${NC} Tailscale: Run ${YELLOW}devbox-status${NC} on VM for Tailscale IP"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"

    # Save instance info
    echo "$instance_name,$zone,$project,$repo_name" >> "$SCRIPT_DIR/.devbox-instances"
}

# ============================================================================
# List running DevBoxes
# ============================================================================
cmd_status() {
    check_gcloud

    echo ""
    echo -e "${CYAN}Running DevBox Instances:${NC}"
    echo ""

    gcloud compute instances list --filter="tags.items=devbox" \
        --format="table(name,zone,machineType.basename(),status,networkInterfaces[0].accessConfigs[0].natIP:label=EXTERNAL_IP)"
}

# ============================================================================
# Connect to DevBox
# ============================================================================
cmd_connect() {
    check_gcloud

    local instance_name="$1"
    local zone="${2:-us-central1-a}"

    if [ -z "$instance_name" ]; then
        # Interactive selection
        echo "Select a DevBox to connect:"
        gcloud compute instances list --filter="tags.items=devbox AND status=RUNNING" \
            --format="table[no-heading](name,zone)"

        echo -n "Instance name: "
        read -r instance_name
        echo -n "Zone: "
        read -r zone
    fi

    log_info "Connecting to $instance_name..."
    gcloud compute ssh "$instance_name" --zone="$zone" -- -A
}

# ============================================================================
# Stop DevBox
# ============================================================================
cmd_stop() {
    local instance_name="$1"
    local zone="${2:-us-central1-a}"

    if [ -z "$instance_name" ]; then
        log_error "Usage: devbox stop <instance-name> [zone]"
        exit 1
    fi

    log_info "Stopping $instance_name..."
    gcloud compute instances stop "$instance_name" --zone="$zone"
    log_success "Instance stopped"
}

# ============================================================================
# Start DevBox
# ============================================================================
cmd_start() {
    local instance_name="$1"
    local zone="${2:-us-central1-a}"

    if [ -z "$instance_name" ]; then
        log_error "Usage: devbox start <instance-name> [zone]"
        exit 1
    fi

    log_info "Starting $instance_name..."
    gcloud compute instances start "$instance_name" --zone="$zone"
    log_success "Instance started"
}

# ============================================================================
# Delete DevBox
# ============================================================================
cmd_delete() {
    local instance_name="$1"
    local zone="${2:-us-central1-a}"

    if [ -z "$instance_name" ]; then
        log_error "Usage: devbox delete <instance-name> [zone]"
        exit 1
    fi

    echo -n "Delete $instance_name? This cannot be undone. (y/N): "
    read -r confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        log_warn "Cancelled"
        exit 0
    fi

    log_info "Deleting $instance_name..."
    gcloud compute instances delete "$instance_name" --zone="$zone" --quiet
    log_success "Instance deleted"
}

# ============================================================================
# SSH Config Generator
# ============================================================================
cmd_ssh_config() {
    check_gcloud

    echo ""
    echo "# Add to ~/.ssh/config"
    echo ""

    gcloud compute instances list --filter="tags.items=devbox AND status=RUNNING" \
        --format="csv[no-heading](name,zone,networkInterfaces[0].accessConfigs[0].natIP)" | \
    while IFS=, read -r name zone ip; do
        echo "Host $name"
        echo "    HostName $ip"
        echo "    User devbox"
        echo "    ForwardAgent yes"
        echo "    # Or use: ProxyCommand gcloud compute ssh $name --zone=$zone --tunnel-through-iap --ssh-flag=\"-W localhost:22\""
        echo ""
    done
}

# ============================================================================
# Help
# ============================================================================
cmd_help() {
    echo ""
    echo -e "${CYAN}GCP DevBox - Codespace-style Development Environments${NC}"
    echo ""
    echo "Usage: devbox <command> [options]"
    echo ""
    echo "Commands:"
    echo "  list              List available repo templates"
    echo "  create            Create a new DevBox instance"
    echo "  status            Show running DevBox instances"
    echo "  connect <name>    SSH into a DevBox"
    echo "  start <name>      Start a stopped DevBox"
    echo "  stop <name>       Stop a running DevBox"
    echo "  delete <name>     Delete a DevBox"
    echo "  ssh-config        Generate SSH config for all DevBoxes"
    echo ""
    echo "Create Options:"
    echo "  -r, --repo <name>       Repository template name"
    echo "  -m, --machine <type>    Machine type (e.g., e2-standard-4)"
    echo "  -z, --zone <zone>       GCP zone (default: us-central1-a)"
    echo "  -p, --project <id>      GCP project ID"
    echo "  -g, --gpu <type>        GPU type (e.g., nvidia-tesla-t4)"
    echo "  -t, --tailscale-key     Tailscale auth key"
    echo "  -d, --disk <size>       Disk size in GB"
    echo "  -i, --interactive       Interactive repo selection"
    echo ""
    echo "Examples:"
    echo "  devbox create -i                    # Interactive mode"
    echo "  devbox create -r claude-flow        # Create claude-flow DevBox"
    echo "  devbox create -r ml-workspace -g nvidia-tesla-t4"
    echo "  devbox connect devbox-claude-flow-20240115"
    echo ""
    echo "Environment Variables:"
    echo "  TAILSCALE_AUTHKEY    Tailscale auth key for auto-connect"
    echo "  GITHUB_TOKEN         GitHub token for private repos"
    echo ""
}

# ============================================================================
# Main
# ============================================================================
main() {
    local command="${1:-help}"
    shift || true

    case "$command" in
        list) cmd_list ;;
        create) cmd_create "$@" ;;
        status) cmd_status ;;
        connect) cmd_connect "$@" ;;
        start) cmd_start "$@" ;;
        stop) cmd_stop "$@" ;;
        delete) cmd_delete "$@" ;;
        ssh-config) cmd_ssh_config ;;
        help|--help|-h) cmd_help ;;
        *) log_error "Unknown command: $command"; cmd_help; exit 1 ;;
    esac
}

main "$@"
