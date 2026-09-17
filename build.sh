#!/usr/bin/env bash
# ==============================================================================
# Antigravity Tools - Linux 编译脚本 (Web Server / Standalone)
# ==============================================================================
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo -e "${CYAN}=====================================================${NC}"
echo -e "${CYAN}     Antigravity Tools Linux 编译脚本 (Web Server)    ${NC}"
echo -e "${CYAN}=====================================================${NC}"

# 1. 检查基础开发工具
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}[ERROR] 未检测到命令: $1${NC}"
        echo -e "${YELLOW}请先安装 $1 后再运行此脚本。${NC}"
        return 1
    fi
}

# 判断是否需要编译前端
NEED_BUILD_FRONTEND=0
if [ "${1:-}" = "--force-frontend" ] || [ ! -d "dist" ] || [ ! -f "dist/index.html" ]; then
    NEED_BUILD_FRONTEND=1
fi

echo -e "\n${YELLOW}[1/4] 检查编译工具链...${NC}"
MISSING_TOOLS=0
REQUIRED_TOOLS="cargo rustc pkg-config gcc"
if [ "$NEED_BUILD_FRONTEND" -eq 1 ]; then
    REQUIRED_TOOLS="node npm $REQUIRED_TOOLS"
fi

for tool in $REQUIRED_TOOLS; do
    if ! check_tool "$tool"; then
        MISSING_TOOLS=1
    fi
done

if [ "$MISSING_TOOLS" -ne 0 ]; then
    echo -e "\n${YELLOW}提示：在基于 Debian / Ubuntu 的系统上，您可以通过以下命令安装必要依赖：${NC}"
    echo -e "  sudo apt-get update && sudo apt-get install -y \\"
    echo -e "    build-essential pkg-config curl wget file libssl-dev \\"
    echo -e "    libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \\"
    echo -e "    librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev"
    echo -e "\n在 Arch Linux 上："
    echo -e "  sudo pacman -S --needed base-devel webkit2gtk-4.1 openssl npm rust"
    echo -e "\n在 Fedora 上："
    echo -e "  sudo dnf install webkit2gtk4.1-devel openssl-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel"
    exit 1
fi
echo -e "${GREEN}✓ 工具链检查通过 ($(cargo --version))${NC}"

# 2 & 3. 前端依赖与构建
if [ "$NEED_BUILD_FRONTEND" -eq 0 ]; then
    echo -e "\n${GREEN}[2/4 & 3/4] 检测到已存在构建好的前端资源 (dist 目录)，自动跳过前端依赖安装与编译。${NC}"
    echo -e "${YELLOW}提示: 如需强制重新编译前端，可运行: ./build.sh --force-frontend${NC}"
else
    echo -e "\n${YELLOW}[2/4] 准备前端依赖 (使用国内高速镜像源)...${NC}"
    if [ ! -d "node_modules" ]; then
        echo -e "正在执行: npm install --legacy-peer-deps --registry=https://registry.npmmirror.com..."
        npm install --legacy-peer-deps --registry=https://registry.npmmirror.com
    else
        echo -e "${GREEN}✓ 前端依赖已存在，跳过 npm install${NC}"
    fi

    echo -e "\n${YELLOW}[3/4] 编译前端静态资源 (Vite)...${NC}"
    npm run build
    if [ ! -d "dist" ]; then
        echo -e "${RED}[ERROR] 前端构建失败: 未生成 dist 目录!${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ 前端构建完成 (dist 目录已就绪)${NC}"
fi

# 4. 编译 Rust 后端
echo -e "\n${YELLOW}[4/4] 编译 Rust 后端 (Cargo Release)...${NC}"
cd "$SCRIPT_DIR/src-tauri"
cargo build --release

cd "$SCRIPT_DIR"
RELEASE_DIR="$SCRIPT_DIR/src-tauri/target/release"
TARGET_BIN="$RELEASE_DIR/antigravity-tools"
DIST_TARGET="$RELEASE_DIR/dist"

if [ ! -f "$TARGET_BIN" ]; then
    echo -e "${RED}[ERROR] 未找到编译生成的二进制文件: $TARGET_BIN${NC}"
    exit 1
fi

# 同步前端静态资源到 release 目录，确保二进制独立部署时也能直接提供 Web UI
echo -e "正在同步前端资源到输出目录: $DIST_TARGET..."
rm -rf "$DIST_TARGET"
cp -r "$SCRIPT_DIR/dist" "$DIST_TARGET"

# 赋予执行权限
chmod +x "$TARGET_BIN"

echo -e "\n${GREEN}=====================================================${NC}"
echo -e "${GREEN}✓ 编译完成！${NC}"
echo -e "${GREEN}可执行文件: ${TARGET_BIN}${NC}"
echo -e "${GREEN}Web 资源目录: ${DIST_TARGET}${NC}"
echo -e "${GREEN}=====================================================${NC}"
echo -e "\n${CYAN}启动方法:${NC}"
echo -e "  cd \"$RELEASE_DIR\""
echo -e "  ./antigravity-tools"
echo -e "\n启动后在浏览器中访问管理面板: ${CYAN}http://localhost:8045${NC}"
