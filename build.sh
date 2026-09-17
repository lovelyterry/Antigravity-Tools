#!/usr/bin/env bash
# ==============================================================================
# Antigravity Tools - Linux 编译脚本 (纯 Headless Axum Web Server 后台服务)
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

# 优先加载 Rustup 环境
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

# 配置 libclang 路径供 bindgen 使用
if [ -z "${LIBCLANG_PATH:-}" ]; then
    for p in /usr/lib/llvm-14/lib /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
        if [ -f "$p/libclang.so" ] || [ -f "$p/libclang.so.1" ]; then
            export LIBCLANG_PATH="$p"
            break
        fi
    done
fi

echo -e "${CYAN}=====================================================${NC}"
echo -e "${CYAN}  Antigravity Tools 编译脚本 (Headless Web Server)   ${NC}"
echo -e "${CYAN}=====================================================${NC}"

# 1. 检查基础开发工具
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}[ERROR] 未检测到命令: $1${NC}"
        echo -e "${YELLOW}请先安装 $1 后再运行此脚本。${NC}"
        return 1
    fi
}

echo -e "\n${YELLOW}[1/4] 检查编译工具链...${NC}"
MISSING_TOOLS=0
for tool in node npm cargo rustc pkg-config gcc g++ cmake clang; do
    if ! check_tool "$tool"; then
        MISSING_TOOLS=1
    fi
done

if [ "$MISSING_TOOLS" -ne 0 ]; then
    echo -e "\n${YELLOW}提示：在基于 Debian / Ubuntu 的系统上，您可以通过以下命令安装必要依赖：${NC}"
    echo -e "  sudo apt-get update && sudo apt-get install -y build-essential cmake clang libclang-dev pkg-config libssl-dev curl wget"
    echo -e "\n在 Arch Linux 上："
    echo -e "  sudo pacman -S --needed base-devel cmake clang openssl npm rust"
    echo -e "\n在 Fedora / RHEL 上："
    echo -e "  sudo dnf install gcc gcc-c++ make cmake clang libclang pkgconfig openssl-devel nodejs npm rust cargo"
    exit 1
fi
echo -e "${GREEN}✓ 工具链检查通过 (Node $(node -v), $(cargo --version))${NC}"

# 2. 安装前端依赖
echo -e "\n${YELLOW}[2/4] 安装前端依赖...${NC}"
NPM_REGISTRY="https://registry.npmmirror.com"
if [ "${1:-}" = "--official-npm" ]; then
    NPM_REGISTRY="https://registry.npmjs.org"
    echo -e "使用官方源: $NPM_REGISTRY"
else
    echo -e "使用国内镜像源加速: $NPM_REGISTRY"
    echo -e "(如需强制使用官方源，可添加参数: ./build.sh --official-npm)"
fi

npm install --legacy-peer-deps --registry="$NPM_REGISTRY"
echo -e "${GREEN}✓ 前端依赖安装完成${NC}"

# 3. 编译前端
echo -e "\n${YELLOW}[3/4] 编译前端静态资源 (Vite)...${NC}"
npm run build
if [ ! -d "dist" ]; then
    echo -e "${RED}[ERROR] 前端构建失败: 未生成 dist 目录!${NC}"
    exit 1
fi
echo -e "${GREEN}✓ 前端构建完成 (dist 目录已生成)${NC}"

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
echo -e "${GREEN}Web 静态资源目录: ${DIST_TARGET}${NC}"
echo -e "${GREEN}=====================================================${NC}"
echo -e "\n${CYAN}启动方法:${NC}"
echo -e "  cd \"$RELEASE_DIR\""
echo -e "  ./antigravity-tools"
echo -e "\n启动后在浏览器中访问管理面板: ${CYAN}http://localhost:8045${NC}"
