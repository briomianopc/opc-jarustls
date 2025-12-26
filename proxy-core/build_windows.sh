#!/bin/bash
# Windows编译脚本 - 用于Google Colab或Linux环境交叉编译Windows可执行文件

set -e

echo "🚀 Starting Windows cross-compilation for proxy-core..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查是否在正确的目录
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}❌ Error: Cargo.toml not found. Please run this script from proxy-core directory.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Found Cargo.toml${NC}"

# 步骤1: 安装Rust（如果未安装）
echo ""
echo "📦 Step 1: Checking Rust installation..."
if ! command -v rustc &> /dev/null; then
    echo -e "${YELLOW}⚠️  Rust not found. Installing...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    echo -e "${GREEN}✅ Rust installed${NC}"
else
    echo -e "${GREEN}✅ Rust already installed: $(rustc --version)${NC}"
fi

# 步骤2: 添加Windows目标
echo ""
echo "🎯 Step 2: Adding Windows target..."
rustup target add x86_64-pc-windows-gnu
echo -e "${GREEN}✅ Windows target added${NC}"

# 步骤3: 安装MinGW交叉编译工具
echo ""
echo "🔧 Step 3: Installing MinGW cross-compiler..."
if command -v apt-get &> /dev/null; then
    # Debian/Ubuntu
    sudo apt-get update
    sudo apt-get install -y mingw-w64
elif command -v yum &> /dev/null; then
    # CentOS/RHEL
    sudo yum install -y mingw64-gcc
elif command -v pacman &> /dev/null; then
    # Arch Linux
    sudo pacman -S --noconfirm mingw-w64-gcc
else
    echo -e "${YELLOW}⚠️  Unknown package manager. Please install mingw-w64 manually.${NC}"
fi
echo -e "${GREEN}✅ MinGW installed${NC}"

# 步骤4: 配置Cargo
echo ""
echo "⚙️  Step 4: Configuring Cargo..."
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
EOF
echo -e "${GREEN}✅ Cargo configured${NC}"

# 步骤5: 编译
echo ""
echo "🔨 Step 5: Building for Windows..."
echo "This may take several minutes..."
cargo build --release --target x86_64-pc-windows-gnu

# 检查编译结果
if [ -f "target/x86_64-pc-windows-gnu/release/proxy-core.exe" ]; then
    echo -e "${GREEN}✅ Build successful!${NC}"
    
    # 获取文件信息
    EXE_PATH="target/x86_64-pc-windows-gnu/release/proxy-core.exe"
    EXE_SIZE=$(du -h "$EXE_PATH" | cut -f1)
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${GREEN}🎉 Compilation completed successfully!${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "📁 Output file:"
    echo "   Path: $EXE_PATH"
    echo "   Size: $EXE_SIZE"
    echo ""
    echo "📋 File details:"
    ls -lh "$EXE_PATH"
    echo ""
    echo "🔍 File type:"
    file "$EXE_PATH"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "📦 To download the file:"
    echo "   1. Copy the file to a download location"
    echo "   2. Or use: cp $EXE_PATH ./proxy-core-windows.exe"
    echo ""
    echo "🚀 To use on Windows:"
    echo "   1. Transfer proxy-core.exe to Windows"
    echo "   2. Run: proxy-core.exe --help"
    echo ""
else
    echo -e "${RED}❌ Build failed. Check the error messages above.${NC}"
    exit 1
fi
