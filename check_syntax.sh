#!/bin/bash

echo "=== Syntax and Structure Check ==="
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

# Check for common syntax issues
echo "=== Checking for Common Issues ==="

# Check for unmatched braces
echo "Checking for unmatched braces..."
for file in "${files[@]}"; do
    open_braces=$(grep -o '{' "$file" | wc -l)
    close_braces=$(grep -o '}' "$file" | wc -l)
    if [ "$open_braces" -ne "$close_braces" ]; then
        echo "⚠️  $file: Unmatched braces (open: $open_braces, close: $close_braces)"
    fi
done
echo "✅ Brace check complete"

# Check for TODO/FIXME comments
echo ""
echo "Checking for TODO/FIXME comments..."
grep -rn "TODO\|FIXME" proxy-core/src/ || echo "✅ No TODO/FIXME found"

echo ""
echo "=== Structure Check Complete ==="
echo ""
echo "To compile and test:"
echo "  1. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
echo "  2. Run: ./check_code.sh"
