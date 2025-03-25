#!/bin/bash
# Program ID Consistency Checker for Anchor Projects
#
# This script validates that the program IDs declared in Rust source files
# match the actual public keys derived from their corresponding keypair files.
# This is crucial for proper deployment and prevents deployment failures.
#
# Usage: ./check-program-id.sh
#
# Exit codes:
#   0: All program IDs match their keypairs
#   1: Inconsistencies found or validation failed

set -euo pipefail

# Configuration
readonly PROGRAMS_DIR="contracts"
readonly DEPLOY_DIR="target/deploy"
readonly SCRIPT_NAME="$(basename "$0")"

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}ℹ️  $*${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $*${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $*${NC}"
}

log_error() {
    echo -e "${RED}❌ $*${NC}"
}

# Function to extract program ID from Rust source file
extract_declare_id() {
    local rs_file="$1"

    if ! grep -q 'declare_id!' "$rs_file"; then
        log_error "No declare_id! macro found in $rs_file"
        return 1
    fi

    # Extract the program ID from declare_id! macro
    grep 'declare_id!' "$rs_file" | sed -E 's/.*"([A-Za-z0-9]+)".*/\1/'
}

# Function to get program ID from keypair file
get_keypair_address() {
    local keypair_file="$1"

    if ! command -v solana &> /dev/null; then
        log_error "Solana CLI not found. Please install Solana CLI tools."
        return 1
    fi

    solana address -k "$keypair_file"
}

# Function to validate a single program
validate_program() {
    local program_path="$1"
    local program_name
    local rs_file
    local keypair_file
    local declare_id
    local actual_id

    program_name=$(basename "$program_path")
    rs_file="$program_path/src/lib.rs"
    keypair_file="$DEPLOY_DIR/$(echo "$program_name" | tr '-' '_')-keypair.json"

    log_info "Validating program: $program_name"

    # Check if source file exists
    if [ ! -f "$rs_file" ]; then
        log_warning "Skipped $program_name - source file not found: $rs_file"
        return 0
    fi

    # Check if keypair file exists
    if [ ! -f "$keypair_file" ]; then
        log_error "Keypair file not found for $program_name: $keypair_file"
        return 1
    fi

    # Extract program IDs
    if ! declare_id=$(extract_declare_id "$rs_file"); then
        log_error "Failed to extract declare_id from $program_name"
        return 1
    fi

    if ! actual_id=$(get_keypair_address "$keypair_file"); then
        log_error "Failed to get address from keypair for $program_name"
        return 1
    fi

    # Compare program IDs
    if [ "$declare_id" = "$actual_id" ]; then
        log_success "$program_name - Program ID matches keypair ($declare_id)"
        return 0
    else
        log_error "$program_name - Program ID mismatch!"
        echo "   Source (declare_id!): $declare_id"
        echo "   Keypair address:      $actual_id"
        echo "   Please update the declare_id! macro or regenerate the keypair"
        return 1
    fi
}

# Main function
main() {
    local mismatch_found=0
    local program_count=0

    log_info "Starting program ID validation..."
    log_info "Programs directory: $PROGRAMS_DIR"
    log_info "Deploy directory: $DEPLOY_DIR"
    echo

    # Check if directories exist
    if [ ! -d "$PROGRAMS_DIR" ]; then
        log_error "Programs directory not found: $PROGRAMS_DIR"
        exit 1
    fi

    if [ ! -d "$DEPLOY_DIR" ]; then
        log_warning "Deploy directory not found: $DEPLOY_DIR"
        log_info "Run 'anchor build' to generate keypair files"
        exit 1
    fi

    # Validate each program
    for program_path in "$PROGRAMS_DIR"/*; do
        if [ -d "$program_path" ]; then
            ((program_count++))
            if ! validate_program "$program_path"; then
                mismatch_found=1
            fi
            echo
        fi
    done

    # Summary
    if [ "$program_count" -eq 0 ]; then
        log_warning "No programs found in $PROGRAMS_DIR"
        exit 0
    fi

    if [ "$mismatch_found" -eq 1 ]; then
        echo
        log_error "Validation failed - inconsistencies found!"
        log_info "To fix inconsistencies:"
        log_info "  1. Update declare_id! macros in source files, OR"
        log_info "  2. Delete keypair files and run 'anchor build' to regenerate them"
        exit 1
    else
        echo
        log_success "All $program_count program(s) validated successfully!"
        log_info "Program IDs are consistent between source files and keypairs"
        exit 0
    fi
}

# Script entry point
main "$@"
