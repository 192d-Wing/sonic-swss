#!/bin/bash
#
# neighsyncd Installation Script
#
# This script installs the SONiC Neighbor Sync Daemon (Rust implementation)
# on a SONiC system with proper permissions, systemd integration, and security.
#
# Usage:
#   sudo ./install.sh [OPTIONS]
#
# Options:
#   --prefix PATH       Installation prefix (default: /usr/local)
#   --sysconfdir PATH   Configuration directory (default: /etc/sonic)
#   --enable-mtls       Generate mTLS certificates (requires OpenSSL)
#   --skip-systemd      Skip systemd service installation
#   --uninstall         Uninstall neighsyncd
#   --help              Show this help message
#
# Requirements:
#   - Root privileges (sudo)
#   - Rust toolchain (for building)
#   - Redis server installed
#   - OpenSSL (for certificate generation)

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
PREFIX="/usr/local"
SYSCONFDIR="/etc/sonic"
ENABLE_MTLS="false"
SKIP_SYSTEMD="false"
UNINSTALL="false"

# Derived paths
BINDIR="${PREFIX}/bin"
CONFDIR="${SYSCONFDIR}/neighsyncd"
METRICSDIR="${SYSCONFDIR}/metrics"
LOGDIR="/var/log/sonic/neighsyncd"
RUNDIR="/var/run/sonic/neighsyncd"
SYSTEMD_UNIT_DIR="/etc/systemd/system"

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# User and group
SONIC_USER="sonic"
SONIC_GROUP="sonic"

# =============================================================================
# Helper Functions
# =============================================================================

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

show_help() {
    sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# //' | head -n -1
    exit 0
}

check_root() {
    if [ "$(id -u)" -ne 0 ]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

check_dependencies() {
    print_info "Checking dependencies..."

    # Check for cargo
    if ! command -v cargo &> /dev/null; then
        print_error "Rust toolchain not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    # Check for Redis (optional, can be on remote host)
    if ! command -v redis-cli &> /dev/null; then
        print_warning "redis-cli not found. Ensure Redis is available on the configured host."
    fi

    # Check for OpenSSL (for certificate generation)
    if [ "${ENABLE_MTLS}" = "true" ]; then
        if ! command -v openssl &> /dev/null; then
            print_error "OpenSSL not found. Required for --enable-mtls. Install with: sudo apt-get install openssl"
            exit 1
        fi
    fi

    print_success "All dependencies satisfied"
}

create_user() {
    if ! id -u ${SONIC_USER} &> /dev/null; then
        print_info "Creating ${SONIC_USER} user..."
        useradd -r -s /bin/false -d /var/run/sonic -c "SONiC Daemon User" ${SONIC_USER}
        print_success "User ${SONIC_USER} created"
    else
        print_info "User ${SONIC_USER} already exists"
    fi

    # Add to required groups
    usermod -a -G adm ${SONIC_USER} 2>/dev/null || true
}

create_directories() {
    print_info "Creating directory structure..."

    # Configuration directory
    mkdir -p "${CONFDIR}"
    chown ${SONIC_USER}:${SONIC_GROUP} "${CONFDIR}"
    chmod 0750 "${CONFDIR}"

    # Log directory
    mkdir -p "${LOGDIR}"
    chown ${SONIC_USER}:${SONIC_GROUP} "${LOGDIR}"
    chmod 0755 "${LOGDIR}"

    # Runtime directory
    mkdir -p "${RUNDIR}"
    chown ${SONIC_USER}:${SONIC_GROUP} "${RUNDIR}"
    chmod 0755 "${RUNDIR}"

    # Metrics certificate directories
    if [ "${ENABLE_MTLS}" = "true" ]; then
        mkdir -p "${METRICSDIR}/ca"
        mkdir -p "${METRICSDIR}/server"
        mkdir -p "${METRICSDIR}/clients/prometheus"
        chmod 0755 "${METRICSDIR}"/{ca,server,clients}
    fi

    print_success "Directories created"
}

build_binary() {
    print_info "Building neighsyncd in release mode..."

    cd "${WORKSPACE_ROOT}"
    cargo build --release -p sonic-neighsyncd

    if [ ! -f "target/release/sonic-neighsyncd" ]; then
        print_error "Build failed: binary not found"
        exit 1
    fi

    print_success "Binary built successfully"
}

install_binary() {
    print_info "Installing binary to ${BINDIR}..."

    mkdir -p "${BINDIR}"
    cp "${WORKSPACE_ROOT}/target/release/sonic-neighsyncd" "${BINDIR}/"
    chmod 0755 "${BINDIR}/sonic-neighsyncd"
    chown root:root "${BINDIR}/sonic-neighsyncd"

    # Verify installation
    if ! "${BINDIR}/sonic-neighsyncd" --version &> /dev/null; then
        print_error "Binary installation verification failed"
        exit 1
    fi

    print_success "Binary installed to ${BINDIR}/sonic-neighsyncd"
}

install_config() {
    print_info "Installing configuration file..."

    local config_file="${CONFDIR}/neighsyncd.conf"

    if [ -f "${config_file}" ]; then
        print_warning "Configuration file already exists at ${config_file}"
        print_info "Backing up to ${config_file}.backup"
        cp "${config_file}" "${config_file}.backup"
    fi

    cp "${SCRIPT_DIR}/neighsyncd.conf.example" "${config_file}"
    chown ${SONIC_USER}:${SONIC_GROUP} "${config_file}"
    chmod 0640 "${config_file}"

    print_success "Configuration installed to ${config_file}"
}

generate_certificates() {
    if [ "${ENABLE_MTLS}" != "true" ]; then
        return
    fi

    print_info "Generating CNSA 2.0 compliant mTLS certificates..."

    # CA Certificate
    if [ ! -f "${METRICSDIR}/ca/ca-key.pem" ]; then
        print_info "Generating CA certificate..."
        openssl ecparam -name secp384r1 -genkey -noout -out "${METRICSDIR}/ca/ca-key.pem"
        chmod 0400 "${METRICSDIR}/ca/ca-key.pem"
        chown root:root "${METRICSDIR}/ca/ca-key.pem"

        openssl req -new -x509 -sha384 -key "${METRICSDIR}/ca/ca-key.pem" \
            -out "${METRICSDIR}/ca/ca-cert.pem" -days 3650 \
            -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=Network Operations/CN=SONiC Metrics CA"

        chmod 0644 "${METRICSDIR}/ca/ca-cert.pem"
        print_success "CA certificate generated"
    else
        print_info "CA certificate already exists"
    fi

    # Server Certificate
    if [ ! -f "${METRICSDIR}/server/server-key.pem" ]; then
        print_info "Generating server certificate..."
        openssl ecparam -name secp384r1 -genkey -noout -out "${METRICSDIR}/server/server-key.pem"
        chmod 0600 "${METRICSDIR}/server/server-key.pem"
        chown ${SONIC_USER}:${SONIC_GROUP} "${METRICSDIR}/server/server-key.pem"

        openssl req -new -sha384 -key "${METRICSDIR}/server/server-key.pem" \
            -out "${METRICSDIR}/server/server.csr" \
            -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=neighsyncd/CN=neighsyncd-metrics"

        cat > "${METRICSDIR}/server/server-san.ext" <<EOF
subjectAltName = IP:::1,DNS:localhost
extendedKeyUsage = serverAuth
keyUsage = digitalSignature, keyAgreement
EOF

        openssl x509 -req -sha384 -in "${METRICSDIR}/server/server.csr" \
            -CA "${METRICSDIR}/ca/ca-cert.pem" \
            -CAkey "${METRICSDIR}/ca/ca-key.pem" \
            -CAcreateserial -out "${METRICSDIR}/server/server-cert.pem" -days 730 \
            -extfile "${METRICSDIR}/server/server-san.ext"

        chmod 0644 "${METRICSDIR}/server/server-cert.pem"
        chown ${SONIC_USER}:${SONIC_GROUP} "${METRICSDIR}/server/server-cert.pem"

        rm "${METRICSDIR}/server/server.csr" "${METRICSDIR}/server/server-san.ext"
        print_success "Server certificate generated"
    else
        print_info "Server certificate already exists"
    fi

    # Client Certificate (for Prometheus)
    if [ ! -f "${METRICSDIR}/clients/prometheus/client-key.pem" ]; then
        print_info "Generating client certificate (Prometheus)..."
        openssl ecparam -name secp384r1 -genkey -noout -out "${METRICSDIR}/clients/prometheus/client-key.pem"
        chmod 0600 "${METRICSDIR}/clients/prometheus/client-key.pem"

        openssl req -new -sha384 -key "${METRICSDIR}/clients/prometheus/client-key.pem" \
            -out "${METRICSDIR}/clients/prometheus/client.csr" \
            -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=Monitoring/CN=prometheus-scraper"

        cat > "${METRICSDIR}/clients/prometheus/client-ext.ext" <<EOF
extendedKeyUsage = clientAuth
keyUsage = digitalSignature
EOF

        openssl x509 -req -sha384 -in "${METRICSDIR}/clients/prometheus/client.csr" \
            -CA "${METRICSDIR}/ca/ca-cert.pem" \
            -CAkey "${METRICSDIR}/ca/ca-key.pem" \
            -CAcreateserial -out "${METRICSDIR}/clients/prometheus/client-cert.pem" -days 730 \
            -extfile "${METRICSDIR}/clients/prometheus/client-ext.ext"

        chmod 0644 "${METRICSDIR}/clients/prometheus/client-cert.pem"

        rm "${METRICSDIR}/clients/prometheus/client.csr" "${METRICSDIR}/clients/prometheus/client-ext.ext"
        print_success "Client certificate generated"
    else
        print_info "Client certificate already exists"
    fi

    # Verify certificates
    print_info "Verifying certificate chain..."
    if openssl verify -CAfile "${METRICSDIR}/ca/ca-cert.pem" "${METRICSDIR}/server/server-cert.pem" &> /dev/null; then
        print_success "Server certificate verification passed"
    else
        print_error "Server certificate verification failed"
        exit 1
    fi

    if openssl verify -CAfile "${METRICSDIR}/ca/ca-cert.pem" "${METRICSDIR}/clients/prometheus/client-cert.pem" &> /dev/null; then
        print_success "Client certificate verification passed"
    else
        print_error "Client certificate verification failed"
        exit 1
    fi

    print_success "All certificates generated and verified"
    print_info "Client certificates for Prometheus located at: ${METRICSDIR}/clients/prometheus/"
}

install_systemd_service() {
    if [ "${SKIP_SYSTEMD}" = "true" ]; then
        print_info "Skipping systemd service installation"
        return
    fi

    print_info "Installing systemd service..."

    cp "${SCRIPT_DIR}/neighsyncd.service" "${SYSTEMD_UNIT_DIR}/"
    chmod 0644 "${SYSTEMD_UNIT_DIR}/neighsyncd.service"

    systemctl daemon-reload

    # Verify service file
    if systemd-analyze verify "${SYSTEMD_UNIT_DIR}/neighsyncd.service" 2>&1 | grep -i error; then
        print_error "Systemd service file verification failed"
        exit 1
    fi

    print_success "Systemd service installed"
    print_info "Enable with: sudo systemctl enable neighsyncd.service"
    print_info "Start with: sudo systemctl start neighsyncd.service"
}

uninstall() {
    print_info "Uninstalling neighsyncd..."

    # Stop service
    if systemctl is-active --quiet neighsyncd.service; then
        print_info "Stopping neighsyncd service..."
        systemctl stop neighsyncd.service
    fi

    # Disable service
    if systemctl is-enabled --quiet neighsyncd.service 2>/dev/null; then
        print_info "Disabling neighsyncd service..."
        systemctl disable neighsyncd.service
    fi

    # Remove systemd service
    if [ -f "${SYSTEMD_UNIT_DIR}/neighsyncd.service" ]; then
        rm "${SYSTEMD_UNIT_DIR}/neighsyncd.service"
        systemctl daemon-reload
        print_success "Systemd service removed"
    fi

    # Remove binary
    if [ -f "${BINDIR}/sonic-neighsyncd" ]; then
        rm "${BINDIR}/sonic-neighsyncd"
        print_success "Binary removed"
    fi

    # Backup and remove configuration
    if [ -f "${CONFDIR}/neighsyncd.conf" ]; then
        print_info "Backing up configuration to ${CONFDIR}/neighsyncd.conf.uninstall-backup"
        cp "${CONFDIR}/neighsyncd.conf" "${CONFDIR}/neighsyncd.conf.uninstall-backup"
        rm "${CONFDIR}/neighsyncd.conf"
    fi

    print_success "Uninstallation complete"
    print_info "Configuration backup: ${CONFDIR}/neighsyncd.conf.uninstall-backup"
    print_info "Logs preserved: ${LOGDIR}"
    print_info "Certificates preserved: ${METRICSDIR}"
}

# =============================================================================
# Main Installation Flow
# =============================================================================

main() {
    echo ""
    echo "=========================================="
    echo "  neighsyncd Installation Script"
    echo "  SONiC Neighbor Sync Daemon (Rust)"
    echo "=========================================="
    echo ""

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --prefix)
                PREFIX="$2"
                BINDIR="${PREFIX}/bin"
                shift 2
                ;;
            --sysconfdir)
                SYSCONFDIR="$2"
                CONFDIR="${SYSCONFDIR}/neighsyncd"
                METRICSDIR="${SYSCONFDIR}/metrics"
                shift 2
                ;;
            --enable-mtls)
                ENABLE_MTLS="true"
                shift
                ;;
            --skip-systemd)
                SKIP_SYSTEMD="true"
                shift
                ;;
            --uninstall)
                UNINSTALL="true"
                shift
                ;;
            --help)
                show_help
                ;;
            *)
                print_error "Unknown option: $1"
                show_help
                ;;
        esac
    done

    # Check root privileges
    check_root

    # Uninstall mode
    if [ "${UNINSTALL}" = "true" ]; then
        uninstall
        exit 0
    fi

    # Normal installation
    check_dependencies
    create_user
    create_directories
    build_binary
    install_binary
    install_config
    generate_certificates
    install_systemd_service

    echo ""
    echo "=========================================="
    print_success "Installation Complete!"
    echo "=========================================="
    echo ""
    print_info "Binary: ${BINDIR}/sonic-neighsyncd"
    print_info "Configuration: ${CONFDIR}/neighsyncd.conf"
    print_info "Logs: ${LOGDIR}"
    if [ "${ENABLE_MTLS}" = "true" ]; then
        print_info "Certificates: ${METRICSDIR}"
        print_info "Client certificates: ${METRICSDIR}/clients/prometheus/"
    fi
    echo ""
    print_info "Next steps:"
    echo "  1. Review and customize: ${CONFDIR}/neighsyncd.conf"
    if [ "${ENABLE_MTLS}" = "true" ]; then
        echo "  2. Distribute client certificates to Prometheus server"
        echo "  3. Configure Prometheus scrape config (see DEPLOYMENT.md)"
    fi
    if [ "${SKIP_SYSTEMD}" != "true" ]; then
        echo "  4. Enable service: sudo systemctl enable neighsyncd.service"
        echo "  5. Start service: sudo systemctl start neighsyncd.service"
        echo "  6. Check status: sudo systemctl status neighsyncd.service"
    fi
    echo ""
    print_info "Documentation: docs/rust/neighsyncd/"
    echo ""
}

# Run main function
main "$@"
