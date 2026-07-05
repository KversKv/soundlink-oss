# 02 · 开发环境搭建 · 桌面端（Tauri 2 + Rust）

桌面端为接收器（Receiver，阶段 1）及后续发送端（Sender，阶段 5），技术栈 **Tauri 2 + Rust (tokio) + React/TS**。可在 Windows / macOS / Linux 上开发。

先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 通用依赖（所有桌面平台）

| 依赖 | 说明 |
|---|---|
| Rust（rustup） | 稳定版工具链 |
| Node.js LTS | 前端构建 |
| pnpm / npm | 前端包管理 |
| Tauri CLI | `cargo install tauri-cli` 或 `pnpm add -D @tauri-apps/cli` |

安装 Rust：

```bash
# Windows (PowerShell) / macOS / Linux 均可用官方安装脚本
# Windows: 下载 rustup-init.exe；或
winget install Rustlang.Rustup
# macOS / Linux:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

校验：

```bash
rustc --version
cargo --version
```

## 2. Windows 专属

- 安装 **Visual Studio Build Tools**（含 “使用 C++ 的桌面开发” 工作负载，提供 MSVC 与 Windows SDK）。
- 安装 **WebView2 Runtime**（Win11 通常已内置；Win10 需手动安装）。
- 音频输出使用 **WASAPI**（`IAudioClient3` / `IAudioRenderClient`），无需额外 SDK，随 Windows SDK 提供。

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Microsoft.EdgeWebView2Runtime
```

## 3. macOS 专属

- 安装 **Xcode Command Line Tools**（提供 clang、CoreAudio 头文件）。
- 音频输出使用 **CoreAudio / AudioUnit**，随系统提供。

```bash
xcode-select --install
```

## 4. Linux 专属（后续阶段）

- 安装 Tauri 所需的 WebKitGTK 及构建依赖（以 Debian/Ubuntu 为例）：

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

- 音频输出优先 **PipeWire**（需 `libpipewire` 开发包）。第一版可不做，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 5. 目录与运行

Rust 核心位于 [`desktop/src-tauri/src`](../../desktop/src-tauri/src)，前端位于 [`desktop/ui/src`](../../desktop/ui/src)。

> 脚手架就绪后，安装前端依赖并启动开发模式：

```bash
cd desktop/ui
pnpm install          # 或 npm install

# 回到 src-tauri 目录启动 Tauri 开发模式（热重载）
cd ../src-tauri
cargo tauri dev       # 或在 ui 目录 pnpm tauri dev
```

编译与打包方式见 [05-build.md](./05-build.md)，调试见 [06-debug.md](./06-debug.md)。

## 6. Lint / 格式化

```bash
cargo fmt
cargo clippy --all-targets
# 前端
cd desktop/ui && pnpm lint
```
