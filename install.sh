#!/bin/bash
# filepath: /home/brean/projects/cleaner/install.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# GitHub repository
REPO="Brean-dev/cleaner"
GITHUB_API="https://api.github.com/repos/$REPO"

echo -e "${GREEN}Downloading 'cleaner' package${NC}"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect architecture
detect_arch() {
    local arch=$(uname -m)
    case $arch in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        *)
            print_error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac
}

# Detect distribution
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo $ID
    elif [ -f /etc/redhat-release ]; then
        echo "rhel"
    elif [ -f /etc/debian_version ]; then
        echo "debian"
    else
        print_error "Cannot detect Linux distribution"
        exit 1
    fi
}

# Get latest release tag
get_latest_release() {
    local response=$(curl -s "$GITHUB_API/releases/latest")
    if [ $? -ne 0 ] || [ -z "$response" ]; then
        print_error "Failed to fetch release information from GitHub API"
        return 1
    fi
    echo "$response" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install based on distro
install_cleaner() {
    local arch=$(detect_arch)
    local distro=$(detect_distro)

    print_status "Detected architecture: $arch"
    print_status "Detected distribution: $distro"

    local version=$(get_latest_release)
    if [ -z "$version" ]; then
        print_error "Failed to get latest release version"
        exit 1
    fi

    print_status "Latest version: $version"

    local base_url="https://github.com/$REPO/releases/download/$version"
    local filename=""
    local install_cmd=""

    case $distro in
        ubuntu|debian|linuxmint)
            if [ "$arch" = "x86_64" ]; then
                filename="cleaner_${version#v}_amd64.deb"
            else
                print_error "No .deb package available for $arch architecture"
                exit 1
            fi
            install_cmd="sudo dpkg -i"
            ;;
        fedora|rhel|centos|rocky|almalinux|opensuse*)
            if [ "$arch" = "x86_64" ]; then
                filename="cleaner-${version#v}-1.x86_64.rpm"
            else
                print_error "No .rpm package available for $arch architecture"
                exit 1
            fi
            if command -v dnf &> /dev/null; then
                install_cmd="sudo dnf install -y"
            elif command -v yum &> /dev/null; then
                install_cmd="sudo yum install -y"
            elif command -v zypper &> /dev/null; then
                install_cmd="sudo zypper install -y"
            else
                install_cmd="sudo rpm -i"
            fi
            ;;
        arch|manjaro)
            # For Arch-based distros, use the binary
            filename="cleaner-$arch-linux"
            install_cmd="install_binary"
            ;;
        *)
            # Default to binary for other distros
            print_warning "Distribution not specifically supported, using binary installation"
            filename="cleaner-$arch-linux"
            install_cmd="install_binary"
            ;;
    esac

    local download_url="$base_url/$filename"
    local temp_file="/tmp/$filename"

    print_status "Downloading $filename from $download_url..."
    if ! curl -L -f -o "$temp_file" "$download_url"; then
        print_error "Failed to download $filename"
        print_error "URL: $download_url"
        exit 1
    fi

    print_status "Installing cleaner..."

    if [ "$install_cmd" = "install_binary" ]; then
        # Install binary directly
        if ! sudo install -m 755 "$temp_file" /usr/local/bin/cleaner; then
            print_error "Failed to install binary to /usr/local/bin/cleaner"
            rm -f "$temp_file"
            exit 1
        fi
        print_status "Cleaner installed to /usr/local/bin/cleaner"
    else
        # Install package
        if ! $install_cmd "$temp_file"; then
            print_error "Failed to install package with: $install_cmd"
            rm -f "$temp_file"
            exit 1
        fi
    fi

    # Clean up
    rm -f "$temp_file"

    print_status "Installation completed successfully!"
    print_status "You can now run 'cleaner' command"

    # Verify installation
    if command -v cleaner &> /dev/null; then
        local version_output=$(cleaner --version 2>/dev/null || cleaner -V 2>/dev/null || echo 'cleaner command is available')
        print_status "Verification: $version_output"
    else
        print_warning "cleaner command not found in PATH. You may need to restart your shell or add /usr/local/bin to your PATH"
        print_status "Try running: export PATH=\"/usr/local/bin:\$PATH\""
    fi
}

# Check for required tools
check_dependencies() {
    local deps=("curl" "grep" "sed")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            print_error "Required dependency '$dep' is not installed"
            exit 1
        fi
    done
}

# Main execution
main() {
    print_status "Starting installation process..."
    check_dependencies
    install_cleaner
}

# Run if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
