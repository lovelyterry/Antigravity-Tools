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

# 解析参数
AUTO_INSTALL=0
OFFICIAL_NPM=0
for arg in "$@"; do
    case "$arg" in
        --auto-install|--install-deps|-y)
            AUTO_INSTALL=1
            ;;
        --official-npm)
            OFFICIAL_NPM=1
            ;;
        --help|-h)
            echo "用法: ./build.sh [选项]"
            echo "选项:"
            echo "  --auto-install, --install-deps, -y  自动检测并安装缺失的编译依赖与 Rust 工具链"
            echo "  --official-npm                      使用 npm 官方源代替国内镜像加速源"
            echo "  --help, -h                          显示此帮助信息"
            exit 0
            ;;
    esac
done

load_rust_env() {
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    elif [ -d "$HOME/.cargo/bin" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
}

load_rust_env

# 配置 libclang 路径供 bindgen 使用
setup_libclang_path() {
    if [ -z "${LIBCLANG_PATH:-}" ]; then
        for p in /usr/lib/llvm-*/lib /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
            if [ -f "$p/libclang.so" ] || [ -f "$p/libclang.so.1" ]; then
                export LIBCLANG_PATH="$p"
                break
            fi
        done
    fi
}
setup_libclang_path

echo -e "${CYAN}=====================================================${NC}"
echo -e "${CYAN}  Antigravity Tools 编译脚本 (Headless Web Server)   ${NC}"
echo -e "${CYAN}=====================================================${NC}"

# 自动安装系统依赖与 Rust 工具链
install_dependencies() {
    echo -e "\n${CYAN}>>> 开始自动安装编译依赖...${NC}"

    # 确定是否需要 sudo
    SUDO=""
    if [ "$(id -u)" -ne 0 ]; then
        if command -v sudo &>/dev/null; then
            SUDO="sudo"
        else
            echo -e "${RED}[ERROR] 当前非 root 用户且未检测到 sudo，无法自动安装系统软件包。${NC}"
            return 1
        fi
    fi

    # 1. 识别包管理器并安装系统依赖
    if command -v apt-get &>/dev/null; then
        echo -e "${YELLOW}检测到 Debian / Ubuntu 系统，通过 apt-get 安装系统依赖...${NC}"
        # 禁用交互提示与 needrestart 服务重启 (避免弹出 TUI 窗口或重启运行中的系统服务)
        export DEBIAN_FRONTEND=noninteractive
        export NEEDRESTART_MODE=l
        export NEEDRESTART_SUSPEND=1
        $SUDO env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=l NEEDRESTART_SUSPEND=1 apt-get update -y
        $SUDO env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=l NEEDRESTART_SUSPEND=1 apt-get install -y \
            -o Dpkg::Options::="--force-confdef" \
            -o Dpkg::Options::="--force-confold" \
            build-essential cmake clang libclang-dev pkg-config libssl-dev curl wget
    elif command -v pacman &>/dev/null; then
        echo -e "${YELLOW}检测到 Arch Linux 系统，通过 pacman 安装系统依赖...${NC}"
        $SUDO pacman -S --needed --noconfirm base-devel cmake clang openssl pkgconf curl wget
    elif command -v dnf &>/dev/null; then
        echo -e "${YELLOW}检测到 Fedora / RHEL 系统，通过 dnf 安装系统依赖...${NC}"
        $SUDO dnf install -y gcc gcc-c++ make cmake clang libclang pkgconfig openssl-devel curl wget
    elif command -v yum &>/dev/null; then
        echo -e "${YELLOW}检测到 CentOS / RHEL 系统，通过 yum 安装系统依赖...${NC}"
        $SUDO yum install -y gcc gcc-c++ make cmake clang pkgconfig openssl-devel curl wget
    else
        echo -e "${YELLOW}[WARN] 未识别的包管理器，跳过系统软件包自动安装。${NC}"
    fi

    # 2. 检查并安装 Rust / Cargo
    load_rust_env
    if ! command -v cargo &>/dev/null || ! command -v rustc &>/dev/null; then
        echo -e "${YELLOW}未检测到 Rust 工具链，正在通过 rustup 自动安装...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        load_rust_env
        echo -e "${GREEN}✓ Rust 工具链安装成功 ($(cargo --version))${NC}"
    fi

    # 重新配置 libclang
    setup_libclang_path
    echo -e "${GREEN}✓ 依赖项自动安装与配置完毕！${NC}\n"
}

# 1. 检查基础开发工具
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}[ERROR] 未检测到命令: $1${NC}"
        return 1
    fi
}

echo -e "\n${YELLOW}[1/4] 检查编译工具链...${NC}"
MISSING_TOOLS=0
REQUIRED_TOOLS=(node npm cargo rustc pkg-config gcc g++ cmake clang)
for tool in "${REQUIRED_TOOLS[@]}"; do
    if ! check_tool "$tool"; then
        MISSING_TOOLS=1
    fi
done

if [ "$MISSING_TOOLS" -ne 0 ]; then
    # 判断是否执行自动安装
    DO_INSTALL=0
    if [ "$AUTO_INSTALL" -eq 1 ]; then
        DO_INSTALL=1
    elif [ -t 0 ]; then
        echo -e "\n${CYAN}是否允许脚本尝试自动安装缺失的依赖项与 Rust 工具链? [Y/n] ${NC}"
        read -r -p "" choice || choice="y"
        case "$choice" in
            [yY][eE][sS]|[yY]|"")
                DO_INSTALL=1
                ;;
            *)
                DO_INSTALL=0
                ;;
        esac
    else
        # 非交互式环境且未显式指定参数
        DO_INSTALL=1
    fi

    if [ "$DO_INSTALL" -eq 1 ]; then
        install_dependencies
        # 重新检验
        load_rust_env
        for tool in "${REQUIRED_TOOLS[@]}"; do
            if ! check_tool "$tool"; then
                echo -e "${RED}[ERROR] 自动安装后仍缺失命令: $tool，请手动安装后重试。${NC}"
                exit 1
            fi
        done
    else
        echo -e "\n${YELLOW}您可使用以下命令手动安装必要依赖：${NC}"
        echo -e "  Debian/Ubuntu: sudo apt-get update && sudo apt-get install -y build-essential cmake clang libclang-dev pkg-config libssl-dev curl wget"
        echo -e "  Rust 工具链:   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
        echo -e "\n或直接运行带有自动安装参数的脚本: ./build.sh --auto-install"
        exit 1
    fi
fi

load_rust_env
setup_libclang_path
echo -e "${GREEN}✓ 工具链检查通过 (Node $(node -v), $(cargo --version))${NC}"

# 2. 安装前端依赖
echo -e "\n${YELLOW}[2/4] 安装前端依赖...${NC}"
NPM_REGISTRY="https://registry.npmmirror.com"
if [ "$OFFICIAL_NPM" -eq 1 ]; then
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
