#!/bin/bash
set -e

OS="$(uname -s)"
ARCH="$(uname -m)"

build_linux() {
    echo "Building pact for Linux (amd64)..."
    cargo build --release --target x86_64-unknown-linux-gnu
    cp target/x86_64-unknown-linux-gnu/release/pact pact-linux-amd64
    chmod +x pact-linux-amd64
    echo "Created pact-linux-amd64"
}

build_macos() {
    echo "Building pact for macOS (arm64)..."
    cargo build --release --target aarch64-apple-darwin
    cp target/aarch64-apple-darwin/release/pact pact-macos-arm64
    chmod +x pact-macos-arm64
    echo "Created pact-macos-arm64"
}

case "$OS" in
    Linux)
        build_linux
        ;;
    Darwin)
        build_macos
        ;;
    *)
        echo "Unknown OS: $OS"
        exit 1
        ;;
esac

echo "Done! Upload the binary to your GitHub release."
