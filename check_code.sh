#!/bin/bash

set -e

echo "=== Code Check Script ==="
echo ""

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo "❌ Rust is not installed"
    echo "To install Rust, run:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust version: $(rustc --version)"
echo "✅ Cargo version: $(cargo --version)"
echo ""

# Check directory structure
echo "=== Checking Directory Structure ==="
if [ ! -d "rustls" ]; then
    echo "❌ rustls directory not found"
    exit 1
fi
echo "✅ rustls directory exists"

if [ ! -d "proxy-core" ]; then
    echo "❌ proxy-core directory not found"
    exit 1
fi
echo "✅ proxy-core directory exists"

if [ ! -f "proxy-core/Cargo.toml" ]; then
    echo "❌ proxy-core/Cargo.toml not found"
    exit 1
fi
echo "✅ proxy-core/Cargo.toml exists"
echo ""

# Check key source files
echo "=== Checking Key Source Files ==="
files=(
    "proxy-core/src/main.rs"
    "proxy-core/src/tls/mod.rs"
    "proxy-core/src/tunnel/mod.rs"
    "proxy-core/src/tunnel/yamux.rs"
    "proxy-core/src/tunnel/websocket.rs"
    "proxy-core/src/doh/mod.rs"
    "proxy-core/src/proxy/mod.rs"
    "proxy-core/src/proxy/socks5.rs"
    "proxy-core/src/proxy/http.rs"
)

for file in "${files[@]}"; do
    if [ ! -f "$file" ]; then
        echo "❌ $file not found"
        exit 1
    fi
    echo "✅ $file exists"
done
echo ""

# Check for syntax errors (without building)
echo "=== Running cargo check ==="
cd proxy-core
if cargo check 2>&1 | tee /tmp/cargo_check.log; then
    echo "✅ cargo check passed"
else
    echo "❌ cargo check failed"
    echo ""
    echo "=== Error Summary ==="
    grep "error" /tmp/cargo_check.log || true
    exit 1
fi
echo ""

# Try to build
echo "=== Running cargo build ==="
if cargo build 2>&1 | tee /tmp/cargo_build.log; then
    echo "✅ cargo build passed"
else
    echo "❌ cargo build failed"
    echo ""
    echo "=== Error Summary ==="
    grep "error" /tmp/cargo_build.log || true
    exit 1
fi
echo ""

echo "=== All Checks Passed ==="
echo ""
echo "Binary location: proxy-core/target/debug/proxy-core"
echo ""
echo "To run the proxy:"
echo "  cd proxy-core"
echo "  cargo run -- --help"
