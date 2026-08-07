# 09 · open-core 双仓库维护、编译与调试

> 面向：项目作者本人（维护两个仓库）、想自行编译的社区贡献者。
> 决策与红线见 [`../NewFunctions/monetization/01-engineering-plan.md`](../NewFunctions/monetization/01-engineering-plan.md)（MON-01）与 [`../NewFunctions/monetization/02-multi-repo-guide.md`](../NewFunctions/monetization/02-multi-repo-guide.md)（含构建硬约束 G10/G11 与实测结论）。
> 本文是**操作手册**：怎么维护、怎么编译、怎么调试。权威构建说明以上述两份文档为准，冲突时以其为准。

---

## 1. 这是什么模型

SoundLink 采用 **open-core**：

- **核心音频流转永久免费、完整开源（MIT）**。公开仓库 `cargo build` 产出的就是完整可用的免费版，无残缺、无「此处省略」。
- **Pro 是自动化与便捷性增强（闭源买断）**，其实现代码放在**独立私有仓库**，公开仓库编译不出来。

实现方式：公开仓库的 `desktop/pro/` 目录放**免费实现**；私有仓库放**同名 crate 的 Pro 实现**。构建免费版还是 Pro 版，取决于 `desktop/pro/` 目录里当前放的是哪份——**构建命令完全相同**。

```
公开仓库 SoundLink（MIT）                     私有仓库 soundlink-pro（闭源）
├─ desktop/pro-api/   soundlink-pro-api ←─ 仅 trait 与类型（两份实现共用）
├─ desktop/pro/       soundlink-pro     ←─ 免费实现（EDITION="community"）
└─ desktop/src-tauri/ soundlink         ←─ 业务代码只调能力，不知 Pro
                                             └─ src/  soundlink-pro ←─ Pro 实现（EDITION="official"）
                                               （crate 名、version 与免费侧完全一致）
```

依赖方向单一：`soundlink → soundlink-pro → soundlink-pro-api`（不可逆）。

> ⚠️ 为什么不用 `--features pro`：Cargo 会解析**可选依赖**并写入 `Cargo.lock`，公开仓库一旦写入私有 git 依赖，无权限者连默认 `cargo build` 都失败（实测确认）。目录替换让公开依赖图里永远只有公开 crate。**不要改回 feature 方案。**

---

## 2. 三个 crate 的职责

| crate | 位置 | 开源 | 职责 | 能写 Pro 逻辑？ |
|---|---|---|---|---|
| `soundlink-pro-api` | 公开 `desktop/pro-api/` | MIT | 定义「有哪些能力」（`ProCapabilities` trait 与类型） | ❌ 只有签名 |
| `soundlink-pro` | 公开 `desktop/pro/`（免费）+ 私有仓库（Pro），同名同版本 | 免费 MIT / Pro 闭源 | 定义「能力值是多少」 | 仅私有版 |
| `soundlink` | 公开 `desktop/src-tauri/` | MIT | 调用能力值干活 | ❌ 禁止 `if is_pro` |

门控只表达为 `ProCapabilities` 返回值（设备上限 / 启动计划 / 重连策略 / 配置档 / 快捷键 / 托盘项）。**业务代码里没有 `if is_pro`**——新增 Pro 能力时给 trait 加方法、两份实现各返回对应值，而不是在业务代码里判断授权。

---

## 3. 免费版：编译与调试（任何人）

### 3.1 编译

```powershell
# 1. 前端依赖
cd desktop\ui
npm ci

# 2. 桌面构建（tauri_app 必须启用，否则 Opus 回退 passthrough 产生噪声）
cd ..\src-tauri
npm exec --prefix ..\ui tauri -- build --features tauri_app
```

产物：`desktop/src-tauri/target/release/soundlink.exe` 与 `target/release/bundle/nsis/*.exe`。

只跑检查与测试：

```powershell
cd desktop\src-tauri
cargo test
cargo clippy --features tauri_app -- -D warnings
```

### 3.2 开发调试（热重载）

```powershell
cd desktop\src-tauri
cargo tauri dev --features tauri_app
```

- Rust 日志：`$env:RUST_LOG="debug"; cargo tauri dev --features tauri_app`（Windows PowerShell）。
- 前端 DevTools：开发模式下右键 → Inspect。
- DEBUG / DUMP_ENABLE 开关见 [06-debug.md](./06-debug.md) §1.5。

### 3.3 社区版与免费能力

社区自行编译得到的就是免费版（`EDITION="community"`）。设置页「授权」区块会显示「本构建不含 Pro（社区版）」，这是**预期行为**，不是 bug——Pro 实现代码不在公开仓库，自行编译无法得到 Pro。

---

## 4. Pro 版：编译与调试（仅作者）

### 4.1 一次性准备

私有仓库放在**公开仓库之外的平级目录**（物理隔离，防误提交）：

```
D:\CodeProject\TRAE_Projects\SoundLink\        公开仓库
D:\CodeProject\TRAE_Projects\soundlink-pro\    私有仓库（独立 git）
```

> ⚠️ 私有仓库**不要**放进公开仓库目录内部（即使加 `.gitignore`），一次 `git add -f` 就泄露。物理隔离是唯一可靠保障（红线 G3）。

### 4.2 本地并行开发（推荐：junction 切换）

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink

# 切到 Pro 开发
Rename-Item -Path desktop\pro -NewName pro-free-backup
New-Item -ItemType Junction -Path desktop\pro -Target D:\CodeProject\TRAE_Projects\soundlink-pro
cargo clean -p soundlink-pro --manifest-path desktop\src-tauri\Cargo.toml

# 切回免费开发
(New-Item -Force -Path desktop\pro).Delete()      # 或 cmd /c rmdir desktop\pro（删链接本体）
Rename-Item -Path desktop\pro-free-backup -NewName pro
cargo clean -p soundlink-pro --manifest-path desktop\src-tauri\Cargo.toml
```

要点（都来自实测，见 02 文档 §11）：

- **`cargo clean -p soundlink-pro` 不是可选步骤**（红线 G10）。替换目录后 crate 名/版本/path 均未变，Cargo 增量缓存**不会察觉源码已变**，会复用旧 `.rlib` 导致「免费目录跑出 Pro 产物」或反之，且**无任何报错**。双向切换都必须执行。
- junction 在 Windows 上**免管理员权限**即可创建，Cargo 正常穿透；私有仓库里的相对路径 `../pro-api` 会按 junction 挂载后的位置解析到公开仓库。
- **删 junction 用 `(New-Item -Force ...).Delete()` 或 `cmd /c rmdir`**，**不要用 `Remove-Item -Recurse -Force`**——后者在部分 PowerShell 版本会穿透 junction 删掉**私有仓库源码**。
- `Rename-Item` 对带横杠的目录名建议用 `-Path/-NewName` 参数形式；位置参数 `Rename-Item A B` 可能报 `PSArgumentException`。

### 4.3 Pro 版构建与打包

切到 Pro 实现后，构建命令与免费版完全相同：

```powershell
cd desktop\src-tauri
cargo clean -p soundlink-pro            # 若刚切换过目录，必须执行
npm exec --prefix ..\ui tauri -- build --features tauri_app --bundles nsis
```

> 注：`tauri.conf.json` 的 `targets:"all"` 时 MSI 目标不支持 `0.1.0-beta.1` 这类预发布号，需发版为正式号或只用 `--bundles nsis`（发布 CI 即走 NSIS）。

验证当前构建产物形态：`desktop\pro\src\lib.rs` 里 `EDITION` 为 `official` 即 Pro-capable 版，`community` 即免费版。官方产物**未激活时行为完全等同免费版**。

### 4.4 两份 soundlink-pro 的版本号必须一致

`Cargo.lock` 会记录 path 依赖的 `version`；两份写不同版本号会导致官方构建无法 `--locked`。**改免费侧 `desktop/pro/Cargo.toml` 的 version 时，必须同步改私有仓库的同名 crate**（红线 G11）。pro crate 的版本号**不参与**根 `VERSION` 同步（不加入 `scripts/sync_version.py` 的 TARGETS）。

---

## 5. CI 双流水线

| 流水线 | 触发 | 用的 `desktop/pro/` | 说明 |
|---|---|---|---|
| 公开 CI（`ci.yml`） | push / PR（含 fork） | 公开免费实现 | 无任何 secret，fork 可全绿；含 license roundtrip 跨语言一致性检查 |
| 发布 CI（`release.yml`） | `v*` tag | 私有实现（token 检出） | 检出后 `cargo clean -p soundlink-pro`，构建官方 Pro-capable 产物 |

发布 CI 检出步骤约束：用**只读、仅该仓库**的 deploy key / 细粒度 PAT（secret 名 `PRO_REPO_TOKEN`）；**不得** `set -x` / echo token / `ls -R desktop/pro`（Pro 源码不进 CI 日志，红线 E7/G9）；fork 的 PR 拿不到 secret，发布 CI 只允许 tag 触发。

---

## 6. 排查表（构建/切换相关）

| 现象 | 原因 | 处理 |
|---|---|---|
| `failed to load source for dependency soundlink-pro` | `desktop/pro/` 不存在或被误删 | `git checkout -- desktop/pro` 恢复免费实现 |
| 替换了 `desktop/pro/` 但产物行为没变 | 增量缓存串味，未清 | `cargo clean -p soundlink-pro`（G10） |
| `Updating soundlink-pro vX -> vY` 后 `--locked` 失败 | 两份 pro crate 版本号不一致 | 统一版本号（G11） |
| `package collision in the lockfile … soundlink-pro-api` | 私有仓库用了绝对路径 path 依赖（Windows 短名） | 改用相对路径 `../pro-api` |
| 激活后仍显示免费 / 「本构建不含 Pro」 | 用的是社区构建（免费实现） | 换官方发布产物，或确认 `desktop/pro` 挂的是私有实现 |
| Pro 构建后 `git status` 出现 `Cargo.lock` 修改 | 私有实现引入了新依赖 | 还原 `Cargo.lock`，**不要提交**（会泄露私有依赖清单，G4） |
| 误删私有仓库源码 | 用 `Remove-Item -Recurse` 删了 junction | 见 §4.2 警示；从远端重新 clone |

完整排查表（含 V-1~V-5 实测失败信息）见 [`02-multi-repo-guide.md`](../NewFunctions/monetization/02-multi-repo-guide.md) §9。

---

## 7. 维护红线（速记）

| # | 禁忌 | 后果 |
|---|---|---|
| G1 | 公开仓库任何提交含私有实现代码/注释/fixture | Pro 逻辑外泄（E7） |
| G2 | 公开仓库出现私有 git 依赖 | 社区无法编译（E3），fork CI 全红 |
| G3 | 私有仓库放进公开仓库内部 | 迟早被 `git add -f` 泄露 |
| G4 | Pro 构建的 `Cargo.lock` 回提公开仓库 | 泄露私有依赖清单 |
| G5 | 在 `soundlink` 里写 `if is_pro` | 门控散落、绕过风险↑ |
| G6 | 用 `EDITION` 常量做门控 | 社区改一个字符串即可伪装 |
| G7 | 把免费实现做成空壳 / `unimplemented!()` | 开源诱饵，违反 E3 |
| G8 | 改 keyring service 名或 `identifier` | 所有存量 license 失效（E8） |
| G10 | 替换 `desktop/pro/` 后不 `cargo clean -p soundlink-pro` | 构建出错版本产物且无报错 |
| G11 | 两份 `soundlink-pro` 版本号不一致 | 官方构建无法 `--locked` |

---

## 关联文档

- 工程改造任务表：[`../NewFunctions/monetization/01-engineering-plan.md`](../NewFunctions/monetization/01-engineering-plan.md)
- 多仓库构建指南（权威，含实测结论）：[`../NewFunctions/monetization/02-multi-repo-guide.md`](../NewFunctions/monetization/02-multi-repo-guide.md)
- 激活码生成与管理：[`10-license-management.md`](./10-license-management.md)
- 各端通用编译/调试：[`05-build.md`](./05-build.md) / [`06-debug.md`](./06-debug.md)
