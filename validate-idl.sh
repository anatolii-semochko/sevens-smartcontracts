#!/bin/bash
# IDL File Validator for Anchor Projects
#
# This script validates Anchor IDL (Interface Definition Language) files
# to ensure they contain required metadata fields, particularly the program address.
# IDL validation is essential for proper client integration and deployment verification.
#
# Usage: ./validate-idl.sh <path-to-idl-file>
#
# Example:
#   ./validate-idl.sh target/idl/my_program.json
#
# Exit codes:
#   0: IDL file is valid and contains required metadata
#   1: IDL file is invalid, missing, or has malformed content

set -euo pipefail

# Configuration
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

# Function to display usage information
usage() {
    cat << EOF
Usage: $SCRIPT_NAME <idl-file>

Validates an Anchor IDL file for required metadata fields.

Arguments:
    idl-file    Path to the IDL JSON file to validate

Options:
    -h, --help  Show this help message

Examples:
    $SCRIPT_NAME target/idl/my_program.json
    $SCRIPT_NAME ../project/target/idl/token_contract.json

The script validates:
    • File exists and is readable
    • JSON structure is valid
    • Contains 'metadata' object
    • Contains 'metadata.address' field
    • Address field is not empty

EOF
}

# Function to validate JSON structure
validate_json_structure() {
    local idl_file="$1"

    if ! command -v jq &> /dev/null; then
        log_warning "jq not found - using basic JSON validation"
        return 0
    fi

    if ! jq empty < "$idl_file" 2>/dev/null; then
        log_error "IDL file contains invalid JSON: $idl_file"
        return 1
    fi

    log_info "JSON structure validation passed"
    return 0
}

# Function to extract program address from IDL file
extract_program_address() {
    local idl_file="$1"
    local address

    # First try with jq if available (more reliable)
    if command -v jq &> /dev/null; then
        if ! address=$(jq -r '.metadata.address // empty' "$idl_file" 2>/dev/null); then
            log_error "Failed to parse IDL file with jq"
            return 1
        fi
    else
        # Fallback to grep/sed method
        if ! grep -q '"metadata"' "$idl_file"; then
            log_error "IDL file missing 'metadata' property"
            return 1
        fi

        if ! grep -q '"address"' "$idl_file"; then
            log_error "IDL file missing 'metadata.address' property"
            return 1
        fi

        # Extract address using grep and sed
        address=$(grep -A 10 '"metadata"' "$idl_file" | grep '"address"' | head -1 | sed -E 's/.*"address": "([^"]+)".*/\1/')
    fi

    # Validate address is not empty
    if [ -z "$address" ] || [ "$address" = "null" ]; then
        log_error "IDL file has empty or null 'metadata.address'"
        return 1
    fi

    # Validate address format (basic Solana address validation)
    if [[ ! "$address" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]]; then
        log_warning "Address format may be invalid: $address"
        log_warning "Expected base58 string of 32-44 characters"
    fi

    echo "$address"
    return 0
}

# Function to extract additional metadata
extract_metadata() {
    local idl_file="$1"

    if ! command -v jq &> /dev/null; then
        return 0
    fi

    local name version
    name=$(jq -r '.name // "Unknown"' "$idl_file" 2>/dev/null)
    version=$(jq -r '.version // "Unknown"' "$idl_file" 2>/dev/null)

    if [ "$name" != "Unknown" ] && [ "$name" != "null" ]; then
        log_info "Program name: $name"
    fi

    if [ "$version" != "Unknown" ] && [ "$version" != "null" ]; then
        log_info "Program version: $version"
    fi
}

# Main validation function
validate_idl() {
    local idl_file="$1"
    local address
    local file_size

    log_info "Starting IDL validation for: $(basename "$idl_file")"
    log_info "File path: $idl_file"

    # Check file existence and readability
    if [ ! -f "$idl_file" ]; then
        log_error "IDL file not found: $idl_file"
        return 1
    fi

    if [ ! -r "$idl_file" ]; then
        log_error "IDL file is not readable: $idl_file"
        return 1
    fi

    # Check file size (basic sanity check)
    file_size=$(stat -f%z "$idl_file" 2>/dev/null || stat -c%s "$idl_file" 2>/dev/null || echo "0")
    if [ "$file_size" -eq 0 ]; then
        log_error "IDL file is empty: $idl_file"
        return 1
    fi

    log_info "File size: $file_size bytes"

    # Validate JSON structure
    if ! validate_json_structure "$idl_file"; then
        return 1
    fi

    # Extract and validate program address
    if ! address=$(extract_program_address "$idl_file"); then
        log_error "Failed to extract or validate program address"
        return 1
    fi

    # Extract additional metadata
    extract_metadata "$idl_file"

    # Success
    log_success "IDL validation completed successfully"
    log_success "Program address: $address"

    return 0
}

# Main function
main() {
    local idl_file

    # Parse command line arguments
    case "${1:-}" in
        -h|--help)
            usage
            exit 0
            ;;
        "")
            log_error "Missing required argument: IDL file path"
            echo
            usage
            exit 1
            ;;
        *)
            idl_file="$1"
            ;;
    esac

    # Perform validation
    if validate_idl "$idl_file"; then
        exit 0
    else
        log_error "IDL validation failed"
        exit 1
    fi
}

# Script entry point
main "$@"
