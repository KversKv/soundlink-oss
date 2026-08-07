# 02 · 开发环境搭建 · 桌面端（Tauri 2 + Rust）

桌面端为接收器（Receiver，阶段 1）及后续发送端（Sender，阶段 5），技术栈 **Tauri 2 + Rust (tokio) + React/TS**。可在 Windows / macOS / Linux 上开发。

先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 通用依赖（所有桌面平台）

| 依赖           | 说明                                                                                                                     |
| ------------ | ---------------------------------------------------------------------------------------------------------------------- |
| Rust（rustup） | 稳定版工具链                                                                                                                 |
| Node.js LTS  | 前端构建                                                                                                                   |
| pnpm / npm   | 前端包管理                                                                                                                  |
| Tauri CLI    | 推荐 `cargo +stable-x86_64-pc-windows-msvc install tauri-cli --version "^2" --locked`；或前端侧 `pnpm add -D @tauri-apps/cli` |

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

推荐安装命令（自动带 C++ 工作负载）：

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Microsoft.EdgeWebView2Runtime -e
rustup toolchain install stable-x86_64-pc-windows-msvc --component rustfmt --component clippy
rustup default stable-x86_64-pc-windows-msvc
```

校验 MSVC 链接器与工具链：

```powershell
where.exe cargo
where.exe cargo-tauri
where.exe link
rustc -vV   # host 应为 x86_64-pc-windows-msvc
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

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink\oss\desktop\src-tauri
npm install           # 或 pnpm install

# 回到 src-tauri 目录启动 Tauri GUI 开发模式（热重载）
cd ..\src-tauri
cargo tauri dev --features tauri_app
```

> 注：本项目默认 `cargo run` / `cargo tauri dev` 不启用 Tauri 外壳，只运行 Rust 核心提示程序；启动 GUI 需显式加 `--features tauri_app`。

编译与打包方式见 [05-build.md](./05-build.md)，调试见 [06-debug.md](./06-debug.md)。

## 6. Lint / 格式化

```bash
cargo fmt
cargo clippy --all-targets
# 前端
cd desktop/ui && pnpm lint
```

> 注：若前端依赖未安装，`vite.config.ts` 会报 "找不到模块 'vite' / '@vitejs/plugin-react'"。先在 `desktop/ui` 下执行 `npm install`（或 `pnpm install`）。

***

## 7. 常见坑（Windows / 国内网络故障排查）

阶段 1 在 Windows 上从零搭建时实际踩到的坑，按现象→原因→修复列出。仅列与**环境/工具链**相关的问题；代码级 bug 不在此。

### 7.1 Rust 工具链

- **现象**：`rustup-init.exe` 双击后卡住、长时间无输出，或 `cargo --version` 报 "linker `link.exe` not found"。
- **原因**：rustup 默认连官方 CDN 慢；或默认工具链选了 MSVC 但未装 VS Build Tools。
- **修复**：
  ```powershell
  # 用清华镜像装 rustup（PowerShell）
  $env:RUSTUP_DIST_SERVER = "https://mirrors.tuna.tsinghua.edu.cn/rustup"
  $env:RUSTUP_UPDATE_ROOT = "https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup"
  # 若已装好但默认工具链不对，显式指定：
  rustup default stable-x86_64-pc-windows-gnu   # 仅核心库/examples
  # 或（推荐，配合 Tauri）
  rustup default stable-x86_64-pc-windows-msvc
  ```
  cargo 镜像写到 `~/.cargo/config.toml`：
  ```toml
  [source.crates-io]
  replace-with = "tuna"
  [source.tuna]
  registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
  ```

### 7.2 MSVC Build Tools（Tauri 二进制必需）

- **现象**：`cargo build --features tauri_app` 报 `link.exe` not found 或大量 `LNK2019`。
- **原因**：Tauri 2 必须用 MSVC 工具链 + Windows SDK；GNU 工具链**不能**编 Tauri 二进制。
- **修复**：
  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools
  # 在安装器里勾选"使用 C++ 的桌面开发"工作负载
  winget install Microsoft.EdgeWebView2Runtime
  rustup default stable-x86_64-pc-windows-msvc
  ```
  临时绕过：仅验证 Rust 核心（lib + examples）可不装 MSVC，用 GNU 工具链：
  ```powershell
  cargo run --example loopback_sender   # 不依赖 Tauri 外壳
  ```

### 7.3 CMake 缺失（启用 `opus` feature）

- **现象**：`cargo build --features opus` 报 `libopus_sys` build script 失败 / `cmake not found`。
- **原因**：`libopus_sys` 用 vendored libopus 1.5，构建需 CMake + C 编译器。
- **修复**：`winget install Kitware.CMake`，确保 `cmake` 在 PATH。

### 7.4 GNU 工具链缺 binutils（`as.exe` / `dlltool.exe`）

- **现象**：用 `stable-x86_64-pc-windows-gnu` 链接时报 `error: linker 'cc' not found` 或找不到 `as.exe`、`dlltool.exe`。
- **原因**：GNU 工具链需要配套的 binutils，rustup 不自带。
- **修复**：装 MSYS2 并安装 mingw-w64 binutils，再把 `C:\msys64\mingw64\bin` 加到 PATH：
  ```powershell
  # MSYS2 已装在 C:\msys64
  & "C:\msys64\usr\bin\bash.exe" -lc "pacman -Sy --noconfirm mingw-w64-x86_64-binutils"
  $env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
  ```
  MSYS2 首次启动会初始化 GPG 密钥环（连 keyserver，可能慢）。镜像换 TUNA：
  编辑 `C:\msys64\etc\pacman.d\mirrorlist.mingw64` 等只留 TUNA 行。

### 7.5 国内网络慢（npm / cargo / pacman）

- **npm**：`npm install --registry=https://registry.npmmirror.com`
- **cargo**：见 7.1 的 `~/.cargo/config.toml`
- **MSYS2 pacman**：`C:\msys64\etc\pacman.d\mirrorlist.*` 改用 TUNA 镜像

### 7.6 rustc / clippy ICE（增量缓存损坏）

- **现象**：`cargo clippy` 报 `thread 'rustc' panicked at .../rmeta/encoder.rs: no entry found for key`。
- **原因**：旧增量编译缓存与新工具链不兼容。
- **修复**：
  ```powershell
  $env:CARGO_INCREMENTAL = "0"
  cargo clean
  cargo clippy --all-targets -- -D warnings
  ```

### 7.7 前端 `vite.config.ts` 报 "找不到模块"

- **现象**：IDE 报 `找不到模块"vite"或其相应的类型声明`。
- **原因**：`desktop/ui/node_modules` 未安装。
- **修复**：`cd desktop/ui && npm install`（或 `pnpm install`）。

### 7.8 MSYS2 首次 `pacman` 卡在密钥环

- **现象**：首次 `pacman -Sy` 卡在 `gpg: 正在更新 ... 密钥`。
- **修复**：耐心等待；若超时，手动初始化：
  ```powershell
  & "C:\msys64\usr\bin\bash.exe" -lc "pacman-key --init && pacman-key --populate msys2"
  ```

### 7.9 `cargo tauri dev` 报 `no such command: tauri`

- **现象**：`cargo tauri dev` 报 `error: no such command: tauri`。
- **原因**：未安装 Rust 版 Tauri CLI（`cargo-tauri.exe`），或 `%USERPROFILE%\.cargo\bin` 不在 PATH。
- **修复**：
  ```powershell
  cd D:\CodeProject\TRAE_Projects\SoundLink\oss\desktop\src-tauri
  cargo +stable-x86_64-pc-windows-msvc install tauri-cli --version "^2" --locked
  cargo tauri --version
  where.exe cargo-tauri
  ```
  若安装成功但命令仍不可见，重新打开 PowerShell/Trae，或确认 PATH 包含：
  ```text
  %USERPROFILE%\.cargo\bin
  ```

### 7.10 安装 `tauri-cli` 报 `gcc.exe not found`

- **现象**：`cargo install tauri-cli` 编译 `ring` 时报：
  ```text
  TARGET = Some(x86_64-pc-windows-gnu)
  failed to find tool "gcc.exe"
  ```
- **原因**：命令在项目外（例如 `C:\Windows\system32`）执行时没有读取项目 `rust-toolchain.toml`，Cargo 使用了系统默认 GNU 工具链；VS Build Tools 不提供 GNU 的 `gcc.exe`。
- **修复**：推荐直接指定 MSVC 工具链安装：
  ```powershell
  rustup toolchain install stable-x86_64-pc-windows-msvc --component rustfmt --component clippy
  cargo +stable-x86_64-pc-windows-msvc install tauri-cli --version "^2" --locked
  ```
  或先切换默认工具链后重新安装：
  ```powershell
  rustup default stable-x86_64-pc-windows-msvc
  rustc -vV   # host 应为 x86_64-pc-windows-msvc
  cargo install tauri-cli --version "^2" --locked
  ```

### 7.11 安装 `tauri-cli` 报 `link.exe not found`

- **现象**：MSVC 工具链下安装/构建时报：
  ```text
  error: linker `link.exe` not found
  ```
- **原因**：已切到 MSVC Rust，但未安装 Visual Studio Build Tools 的 C++ 工作负载，或安装后终端未重启。
- **修复**：
  ```powershell
  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  where.exe link
  ```
  若 `where.exe link` 仍找不到，重新打开 PowerShell/Trae；也可从 “x64 Native Tools Command Prompt for VS 2022” 启动验证。

### 7.12 `beforeDevCommand` 找不到 `ui/package.json`

- **现象**：`cargo tauri dev` 报：
  ```text
  npm error path D:\CodeProject\TRAE_Projects\SoundLink\oss\ui\package.json
  Could not read package.json
  ```
- **原因**：Tauri 2 执行 `beforeDevCommand` 时路径按工作区解析；若配置为 `npm --prefix ../ui run dev`，会错误指向仓库根目录下的 `ui`。
- **修复**：本项目已将 `desktop/src-tauri/tauri.conf.json` 调整为：
  ```json
  "beforeDevCommand": "npm --prefix ./ui run dev",
  "beforeBuildCommand": "npm --prefix ./ui run build"
  ```
  并确认前端依赖已安装：
  ```powershell
  cd D:\CodeProject\TRAE_Projects\SoundLink\oss\desktop\ui
  npm install
  ```

### 7.13 `cargo tauri dev` 只打印“无 Tauri 外壳”后退出

- **现象**：命令能编译运行，但只输出：
  ```text
  SoundLink 桌面核心（无 Tauri 外壳）。
  GUI 外壳： cargo build --features tauri_app
  ```
- **原因**：本项目将 Tauri 外壳放在可选 feature `tauri_app` 下，默认构建只运行 Rust 核心，便于不装 Tauri 环境时自测核心库。
- **修复**：启动 GUI 时加 feature：
  ```powershell
  cd D:\CodeProject\TRAE_Projects\SoundLink\oss\desktop\src-tauri
  cargo tauri dev --features tauri_app
  ```

### 7.14 Trae 沙箱中运行 Tauri 报 AppData `拒绝访问`

- **现象**：在 Trae 代理命令里运行 GUI 时出现：
  ```text
  Failed to setup app: 拒绝访问。 (os error 5)
  TRAE Sandbox Error: hit restricted
  Not allow operate files: C:\Users\...\AppData\Roaming\soundlink, C:\Users\...\AppData\Local\com.soundlink.desktop
  ```
- **原因**：这是 Trae 沙箱文件访问限制，不是 Tauri/项目代码编译失败。Tauri 运行时需要访问系统 AppData 目录保存应用状态。
- **修复**：在普通 PowerShell 中运行：
  ```powershell
  cd D:\CodeProject\TRAE_Projects\SoundLink\oss\desktop\src-tauri
  cargo tauri dev --features tauri_app
  ```
  或调整 Trae 沙箱允许访问对应 AppData 路径后再在 IDE 代理命令中运行。

