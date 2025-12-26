#!/bin/bash
# Build script for ws-proxy-client

set -e

echo "🔨 Building ws-proxy-client..."

# Build release version
cargo build --release

echo "✅ Build complete!"
echo ""
echo "Executable location:"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    echo "  target/release/ws-proxy-client.exe"
else
    echo "  target/release/ws-proxy-client"
fi
echo ""
echo "Run with:"
echo "  ./target/release/ws-proxy-client --config config.toml"
