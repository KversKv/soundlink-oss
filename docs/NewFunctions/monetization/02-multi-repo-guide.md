<!-- MON-02 -->
# 多仓库（open-core）构建与使用指南

> 建档：2026-08-06
> 面向：项目作者本人、未来接手者、以及想自行编译的社区贡献者
> 决策依据见 [`00-monetization-overview.md`](./00-monetization-overview.md) §7；任务编号见 [`01-engineering-plan.md`](./01-engineering-plan.md) 阶段 Q

**本文件描述的是阶段 Q 落地后的目标形态。** 当前代码尚未切分，命令中带 `desktop/pro*` 的部分在 Q 阶段完成前不可用。

---

## 1. 为什么要拆成两个仓库

一句话：**让公开仓库编译出来的是完整可用的免费版，而不是完整版。**

| 目标 | 单仓库能否做到 |
|---|---|
| 核心音频流转开源、可自行编译、无残缺 | ✅ |
| Pro 逻辑不被直接编译获得 | ❌ 代码在公开仓库里，删掉 license 判断即可解锁 |

因此 Pro 的**实现代码**移到私有仓库，公开仓库只保留**能力接口**与**免费实现**。

---

## 2. 仓库与 crate 拓扑

### 2.1 目标形态

```
公开仓库  SoundLink  (MIT, GitHub 公开)
│
├── desktop/pro-api/            crate: soundlink-pro-api      【公开】
│   └── 只有 trait 与数据类型，无业务逻辑
│       ProCapabilities / Entitlement / StartupPlan / ReconnectPolicy ...
│
├── desktop/pro/                crate: soundlink-pro          【公开】← 免费实现
│   └── FreeCapabilities：记 1 台设备、不自动启动、无配置档
│       这是真实合理的降级行为，不是空占位（红线 E3）
│
├── desktop/src-tauri/          crate: soundlink              【公开】
│   └── 免费核心全部逻辑；以 path 依赖上面两个 crate
│
└── desktop/ui/                 前端（免费/Pro 共用同一份）

私有仓库  soundlink-pro  (闭源, 仅作者可访问)
└── crate: soundlink-pro        【私有】← Pro 实现
    crate 名与公开侧 desktop/pro/ **完全相同**
    依赖 soundlink-pro-api（走公开仓库的 git 依赖）
    实现 PRO-1 ~ PRO-5 的真实逻辑
```

### 2.2 三个 crate 的职责

| crate | 位置 | 开源 | 职责 | 允许出现 Pro 逻辑？ |
|---|---|---|---|---|
| `soundlink-pro-api` | 公开仓库 `desktop/pro-api/` | MIT | 定义「有哪些能力」 | ❌ 只有签名与类型 |
| `soundlink-pro` | **两份同名实现** | 公开版 MIT / 私有版闭源 | 定义「能力值是多少」 | 仅私有版 |
| `soundlink` | 公开仓库 `desktop/src-tauri/` | MIT | 调用能力值干活 | ❌ 禁止 `if is_pro` |

**为什么需要 `pro-api` 这第三个 crate**：`soundlink` 需要调用 `soundlink-pro`，而 `soundlink-pro` 又需要 trait 定义。若 trait 放在 `soundlink` 里就会形成循环依赖。抽出独立 crate 后依赖方向单一：

```
soundlink  ──→ soundlink-pro ──→ soundlink-pro-api
    └──────────────────────────────────↗
```

---

## 3. 两种构建形态

### 3.1 切换机制：替换 `desktop/pro/` 目录

`desktop/src-tauri/Cargo.toml` 里恒定写：

```toml
[dependencies]
soundlink-pro-api = { path = "../pro-api" }
soundlink-pro     = { path = "../pro" }
```

**没有 `pro` feature，没有可选依赖。** 构建哪个版本，取决于 `desktop/pro/` 目录里放的是哪份实现：

| 目录内容 | 产物 |
|---|---|
| 公开仓库自带的免费实现 | **社区构建**（纯免费版） |
| 私有仓库检出覆盖后 | **官方构建**（Pro-capable，未激活时行为等同免费版） |

> ⚠️ **为什么不用 `cargo build --features pro` + 可选 git 依赖 —— 已实测证实**
> Cargo 在解析依赖图时会把**可选依赖也一并解析并写入 `Cargo.lock`**，与是否启用 feature 无关。这意味着公开仓库一旦写入私有 git 依赖，**没有访问权的社区用户连默认的 `cargo build` 都会失败**（拉取私有仓库 403），直接违反红线 E3 与「社区 fork 可通过 CI」的目标。
> 目录替换方案完全绕开这个问题：公开仓库的依赖图里永远只有公开 crate。
>
> **实测结论（cargo 1.96.1 / rustc 1.96.1，见 §11 V-1）**：在探针 crate 中把不可访问的 git 仓库写成 `optional = true` 且 `default = []`（`pro` feature 未启用），执行**默认** `cargo build` 仍会 `Updating git repository …` → `Repository not found` → 重试 3 次后 `error: failed to get 'ghost-pro' as a dependency`。用不存在的 `path` 依赖同样失败。**结论与预期一致，目录替换方案确立，不再保留 feature 方案的回退路径。**

> ⚠️ **两份 `soundlink-pro` 的 `version` 字段必须完全一致**（实测得出，见 §5.2）。`Cargo.lock` 会记录 path 依赖的版本号，版本号不同会导致官方构建时 lock 变动，从而与 `--locked` 冲突。

### 3.2 构建免费版（任何人，无需任何凭据）

```powershell
# 1. 前端依赖
cd desktop\ui
npm ci

# 2. 桌面构建（tauri_app feature 必须启用，否则 Opus 回退 passthrough 产生噪声）
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

### 3.3 构建 Pro 版（仅作者 / 有私有仓库权限）

```powershell
# 在仓库根目录执行
# 1. 移除公开的免费实现（不要 git rm，只是工作区替换）
Remove-Item -Recurse -Force desktop\pro

# 2. 检出私有实现到同一位置
git clone git@github.com:<owner>/soundlink-pro.git desktop\pro

# 3. 【必须】清理 soundlink-pro 的增量缓存，否则会沿用上一次构建的实现
cd desktop\src-tauri
cargo clean -p soundlink-pro

# 4. 正常构建
npm exec --prefix ..\ui tauri -- build --features tauri_app
```

恢复免费构建：

```powershell
Remove-Item -Recurse -Force desktop\pro
git -C . checkout -- desktop/pro
cargo clean -p soundlink-pro --manifest-path desktop\src-tauri\Cargo.toml
```

> ⚠️ **`cargo clean -p soundlink-pro` 不是可选步骤**（实测得出，见 §11 V-4）。替换目录后 Cargo 的指纹机制**不会**察觉源码已变（crate 名、版本号、path 均未变），会直接复用 `target/` 里的旧 `.rlib`，导致**免费目录构建出 Pro 产物**或反之。两个方向都会串味，`cargo clean -p soundlink-pro` 双向有效且只需 ~0.3 MB 重编译，代价极低。CI 里因每次都是全新 checkout，无此问题，但**本地与任何复用 `target/` 的构建机必须执行**。

> `desktop/pro/.git`（私有仓库的 git 目录）必须进 `.gitignore` 排除逻辑，见 §7 红线。

### 3.4 本地并行开发（推荐给作者）

不想反复 clone 时，把私有仓库放在**公开仓库之外**，用符号链接切换：

```powershell
# 目录布局
#   D:\CodeProject\TRAE_Projects\SoundLink\          公开仓库
#   D:\CodeProject\TRAE_Projects\soundlink-pro\      私有仓库（平级，独立 git）

# 切到 Pro 开发
Rename-Item desktop\pro desktop\pro-free-backup
New-Item -ItemType Junction -Path desktop\pro -Target D:\CodeProject\TRAE_Projects\soundlink-pro
cargo clean -p soundlink-pro --manifest-path desktop\src-tauri\Cargo.toml

# 切回免费开发
(Get-Item desktop\pro).Delete()          # 删 junction 本身，不碰目标目录
Rename-Item desktop\pro-free-backup desktop\pro
cargo clean -p soundlink-pro --manifest-path desktop\src-tauri\Cargo.toml
```

实测确认（V-2）：Junction 在 Windows 上**不需要管理员权限**即可创建，Cargo 能正常穿透，且私有仓库 `Cargo.toml` 里的 `soundlink-pro-api = { path = "../pro-api" }` 这类相对路径会**按 junction 挂载后的位置解析**（即解析到公开仓库的 `desktop/pro-api`），因此私有仓库可以直接写相对路径、无需硬编码绝对路径。

> 反过来说，私有仓库**无法独立构建**（脱离公开仓库时 `../pro-api` 不存在，实测报 `failed to load source for dependency`）。这是预期行为：私有仓库只有在被挂载进公开仓库时才成立。若需要在私有仓库内单跑 `cargo test`，须自行挂载或临时改 path。

> ⚠️ 删除 junction 用 `(Get-Item …).Delete()` 或 `cmd /c rmdir`，**不要用 `Remove-Item -Recurse -Force`** —— 后者在部分 PowerShell 版本上会穿透 junction 删除**目标目录内容**，即删掉你的私有仓库源码。

> 私有仓库**不要**放在公开仓库目录内部（即使加了 `.gitignore`），一次误操作的 `git add -f` 就会泄露。物理隔离是唯一可靠的保障。

---

## 4. 免费 / Pro 构建的自我标识

`soundlink-pro` 两份实现各自导出一个常量：

```rust
// 公开 desktop/pro/src/lib.rs
pub const EDITION: &str = "community";

// 私有 soundlink-pro/src/lib.rs
pub const EDITION: &str = "official";
```

用途：

| 用途 | 说明 |
|---|---|
| `get_license_status()` 的 `pro_build` 字段（R5） | 社区构建下为 `false`，前端把 Pro 区块显示为「本构建不含 Pro（社区版）」而非「点击购买」 |
| 日志首行 | 启动日志打印 edition，排查用户问题时一眼分辨 |
| 产物文件名 | 见 §6 |

**不得**用 `EDITION` 做门控判断——门控只看 `ProCapabilities` 返回值（红线 E4）。

---

## 5. 版本号与 `Cargo.lock`

### 5.1 版本号：Pro crate 不参与 `VERSION` 同步

| crate | 版本号来源 |
|---|---|
| `soundlink` / `tauri.conf.json` / `ui/package.json` / `pubspec.yaml` | 仓库根 `VERSION` + [`scripts/sync_version.py`](../../../scripts/sync_version.py) |
| `soundlink-pro-api` | 独立手工 semver（接口稳定，极少变） |
| `soundlink-pro`（两份） | 独立手工 semver，**但两份必须写同一个值**（见下方警示） |

理由：`desktop/src-tauri/Cargo.toml` 用 **path 依赖且不写版本约束**，因此 pro crate 的版本号不需要与产品版本对齐。这样 CI 的 `version-check` 门（`sync_version.py --check` 校验 4 个清单）**无需改动**。

> ❗ 不要把 pro crate 加进 `sync_version.py` 的 `TARGETS`。加了就意味着私有仓库也要跟着改版本，等于把发版流程绑死在两个仓库上。

> ⚠️ **两份 `soundlink-pro` 的 `version` 必须完全相同**（实测得出，见 §11 V-3）。`Cargo.lock` 里会记录 path 依赖的 `version` 字段；免费版写 `0.1.0`、私有版写 `2.7.3` 时，官方构建会触发 `Updating soundlink-pro v0.1.0 -> v2.7.3` 并改写 lock，于是**无法使用 `--locked`**。两份写同一个版本号后，官方构建可直接 `--locked` 通过（已实测）。建议在私有仓库 README 与 CI 里都加一条「version 必须与公开 `desktop/pro/Cargo.toml` 一致」的校验。
>
> 另注：私有仓库若用**绝对路径**写 `soundlink-pro-api` 的 path 依赖，会因 Windows 短名（`ADMINI~1`）与长名不一致触发 `error: package collision in the lockfile`。**一律使用相对路径 `../pro-api`。**

### 5.2 `Cargo.lock`：两份版本号一致时可全程 `--locked`

私有 `soundlink-pro` 可能引入公开侧没有的依赖，导致 `Cargo.lock` 变化。规则：

- **公开仓库提交的 `Cargo.lock` 以免费构建为准**，由公开 CI 保证 `--locked` 可通过。
- **若私有实现不引入任何新的外部依赖**（首版即是此情形，只用已有 crate），且两份 `version` 一致，则官方构建**也可以加 `--locked`**（已实测通过）。这是首选状态，应尽量维持。
- **一旦私有实现引入公开侧没有的依赖**，官方构建须去掉 `--locked`，允许 lock 就地更新，且**更新结果绝不回提到公开仓库**（会泄露私有依赖清单 → 红线 E7）。
- 发布 CI 在构建后**不执行任何 git 写操作**。

> 实测的失败信息（V-5）供排查参考：lock 与依赖图不符时报 `error: cannot update the lock file … because --locked was passed to prevent this`，并提示改用 `--offline`。信息足够清晰，无需额外包装。

Pro 构建前先备份：

```powershell
Copy-Item desktop\src-tauri\Cargo.lock $env:TEMP\Cargo.lock.free.bak
# ... 构建 ...
Copy-Item $env:TEMP\Cargo.lock.free.bak desktop\src-tauri\Cargo.lock -Force
```

---

## 6. 分发决策：官方只发一种产物

| 渠道 | 产物 | 内含 Pro 代码 | 未激活时行为 |
|---|---|---|---|
| **GitHub Release / 官网下载** | 官方构建（`EDITION=official`） | ✅ | 完全等同免费版，无水印、无弹窗、无功能倒计时 |
| **社区自行编译** | 社区构建（`EDITION=community`） | ❌ | 免费版 |

**为什么官方只发一种**：用户下载一次即可，买了 key 粘贴就解锁，不需要「重新下载 Pro 版」。这同时让 §5 的更新适配天然成立——**官方产物线只有一条，升级不存在「被换成免费版」的可能**。

产物命名（沿用现有 [`release.yml`](../../../.github/workflows/release.yml) 规则，**不加 `-pro` 后缀**）：

```
SoundLink-<version>-windows-x64-portable.exe
SoundLink_<version>_x64-setup.exe
```

社区 CI 产出的构建**不上传为 Release 资产**，仅作为编译验证。

---

## 7. 使用方（终端用户）视角

用户完全不需要知道多仓库的存在。完整路径：

1. 从 GitHub Release 或官网下载安装 → 得到完整免费版，永久可用。
2. 想要 Pro：设置 → 授权 → 复制设备指纹 → 在爱发电/淘宝下单并提交指纹。
3. 收到 key → 粘贴 → 立即解锁，**无需重启、无需重新下载**。
4. 后续版本升级：正常覆盖安装，license 存在用户配置域，**自动继续有效**（见 [`01-engineering-plan.md`](./01-engineering-plan.md) §4.2）。

想自行编译的用户：按 §3.2 操作，得到的是免费版；这一点必须在 README 写明，不能让人以为自编译能得到 Pro（红线：诚实沟通，00 文档 §7.3）。

---

## 8. CI 双流水线

| 流水线 | 触发 | 用的 `desktop/pro/` | 目的 |
|---|---|---|---|
| **公开 CI**（[`ci.yml`](../../../.github/workflows/ci.yml)） | push / PR（含 fork） | 公开免费实现 | 保证社区 fork 无凭据也能全绿 |
| **发布 CI**（[`release.yml`](../../../.github/workflows/release.yml)） | `v*` tag | 私有实现（deploy key 检出） | 产出官方可售产物 |

发布 CI 新增步骤（对应任务 Q5）：

```yaml
- name: Checkout private Pro implementation
  run: |
    rm -rf desktop/pro
    git clone --depth 1 "https://x-access-token:${{ secrets.PRO_REPO_TOKEN }}@github.com/<owner>/soundlink-pro.git" desktop/pro
    rm -rf desktop/pro/.git
```

约束：

- `PRO_REPO_TOKEN` 用**只读、仅该仓库**的细粒度 PAT 或 deploy key，不用账号级 token。
- 该步骤**不得** `set -x`、不得 echo token、不得 `ls -R desktop/pro`（红线 E7：Pro 源码不进 CI 日志）。
- fork 的 PR 拿不到 secret，因此发布 CI **只允许 tag 触发**，不能被 PR 触发。
- 公开 CI 必须显式不依赖任何 secret，否则 fork 会红。
- **发布 CI 若启用 `Swatinem/rust-cache@v2`**（现有 `ci.yml` 已启用），必须在检出私有实现后加 `cargo clean -p soundlink-pro`——缓存恢复的 `target/` 会带上一次构建的 `soundlink-pro`，触发 V-4 的串味问题。若发布 CI 不用缓存则可省略，但加上无害。
- 两份 pro crate 版本号一致时（§5.1 要求），发布 CI 可保留 `--locked`；一旦私有实现新增依赖，须去掉。

---

## 9. 排查表

| 现象 | 原因 | 处理 |
|---|---|---|
| `error: failed to load source for dependency soundlink-pro` | `desktop/pro/` 不存在或被误删 | `git checkout -- desktop/pro` 恢复免费实现 |
| 社区用户报告 `cargo build` 拉取私有仓库失败 | 公开仓库出现了私有依赖（违反 §3.1） | 立即移除私有 git 依赖，改回 path 依赖 |
| **替换了 `desktop/pro/` 但产物行为没变**（免费目录跑出 Pro 行为，或反之） | Cargo 增量缓存串味，未察觉源码变化（V-4 已实测复现） | `cargo clean -p soundlink-pro`，见 §3.3 |
| `error: package collision in the lockfile: … soundlink-pro-api … are different` | 两处 path 指向同一目录但字符串不同（Windows 短名 `ADMINI~1` vs 长名） | 私有仓库改用相对路径 `../pro-api`，见 §5.1 |
| `Updating soundlink-pro v0.1.0 -> v2.7.3` 后 `--locked` 失败 | 两份 pro crate 的 `version` 不一致 | 统一版本号，见 §5.1 |
| `error: cannot update the lock file … --locked was passed` | 依赖图与提交的 lock 不符 | 若私有实现新增了依赖，去掉 `--locked`，见 §5.2 |
| Pro 构建后 `git status` 出现 `Cargo.lock` 修改 | 私有 crate 引入新依赖 | 按 §5.2 还原，**不要提交** |
| 激活后仍显示免费 | 免费/社区构建（`pro_build=false`） | 确认用的是官方下载产物，而非自编译 |
| 升级后变回免费 | 配置目录被清理，或 keyring 条目丢失 | 重新粘贴 key（用户手上的 key 永久有效）；同时按 U6b 排查安装器行为 |
| 公开 CI 在 fork 上失败 | 流水线用到了 secret | 拆分流水线，公开 CI 去除 secret 依赖 |
| `git add` 误把私有实现加入暂存区 | 私有仓库放在了公开仓库内部 | 按 §3.4 物理隔离；`git rm --cached` 并检查是否已 push |
| 私有仓库源码被删 | 用 `Remove-Item -Recurse -Force` 删了 junction，穿透删了目标 | 见 §3.4 警示；从远端重新 clone |

---

## 10. 红线与禁忌

| # | 禁忌 | 后果 |
|---|---|---|
| G1 | 公开仓库任何提交包含私有实现代码/注释/测试 fixture | Pro 逻辑外泄，商业模型失效（E7） |
| G2 | 公开仓库出现私有 git 依赖 | 社区无法编译（E3），fork CI 全红 |
| G3 | 把私有仓库目录放在公开仓库内部 | 早晚被 `git add -f` 或工具误提交 |
| G4 | Pro 构建产生的 `Cargo.lock` 回提公开仓库 | 泄露私有依赖清单 |
| G5 | 在 `soundlink` 里写 `if is_pro` | 门控散落，绕过风险↑（E4） |
| G6 | 用 `EDITION` 常量做门控 | 同上；且社区改一个字符串即可伪装 |
| G7 | 把免费实现做成空壳 / `unimplemented!()` | 违反 E3，社区会判定为开源诱饵 |
| G8 | 修改 keyring service 名或 `identifier` | 所有存量 license 失效（E8） |
| G9 | CI 日志打印 token 或私有源码 | 凭据泄露 |
| G10 | 替换 `desktop/pro/` 后不执行 `cargo clean -p soundlink-pro` | 构建出错版本产物且**无任何报错**，是本方案最隐蔽的坑（V-4 实测） |
| G11 | 两份 `soundlink-pro` 版本号不一致 | 官方构建无法 `--locked`，lock 每次都变动（V-3 实测） |

---

## 11. 实测结论（已完成，2026-08-06）

环境：`cargo 1.96.1` / `rustc 1.96.1`，Windows。方法：在 `%TEMP%` 建三 crate 探针工程（`slv-pro-api` / `slv-pro` / `slv-app`）复刻目标拓扑，用 junction 做目录替换。

- [x] **V-1 Cargo 会解析未启用的可选依赖 —— 结论成立，目录替换方案确立。**
      `default = []` 且 `pro = ["dep:ghost-pro"]` 未启用时，默认 `cargo build` 仍去 clone 不存在的私有仓库，重试 3 次后 `error: failed to get 'ghost-pro' as a dependency`。不可访问的 `path` 依赖同样失败。→ feature 方案会让社区构建全红，**不保留为回退选项**。
- [x] **V-2 junction 下 path 依赖正常。** Junction 免管理员权限即可创建；Cargo 正常穿透；私有仓库内的相对路径 `../pro-api` 按挂载后位置解析到公开仓库。副产物：私有仓库无法脱离公开仓库独立构建（预期行为）。
- [x] **V-3 同名 crate 可替换，但版本号必须一致。** 版本号不同（`0.1.0` vs `2.7.3`）时构建可成功，但 lock 会记录版本并触发 `Updating … -> v2.7.3`，与 `--locked` 冲突；统一版本号后 `--locked` 直接通过。另发现绝对路径会因 Windows 短名触发 `package collision in the lockfile`。→ 写入 §5.1 约束与 G11。
- [x] **V-4 增量缓存确实会串味（双向）—— 本方案最大的坑。** 替换目录后 crate 名、版本、path 均未变，Cargo 指纹不认为源码变化，直接复用旧 `.rlib`：免费目录跑出 `edition=official`，官方目录跑出 `edition=community`，且**无任何警告**。`cargo clean -p soundlink-pro` 双向均能修正（约 0.3 MB 重编译）。→ 写入 §3.3 必须步骤与 G10。
- [x] **V-5 `--locked` 失败信息清晰。** `error: cannot update the lock file … because --locked was passed to prevent this` + 提示改用 `--offline`。无需额外包装。

未实测项（留待 Q 阶段真实工程内验证，风险低）：

- [ ] **V-6** Tauri NSIS 完整打包在替换目录后的表现（探针只验证了 `cargo build/run` 层面的串味，打包层预期同理，因为 bundle 输入就是 `cargo build` 产物；但需确认 `cargo clean -p` 后 tauri 不会因缺少缓存报错）
- [ ] **V-7** `cargo clippy` 在 junction 下的表现（预期与 `cargo build` 一致，V-2 未单独跑 clippy）

---

## 12. 回填规则

1. §11 的 V-6/V-7 完成后勾选并写结论。V-1~V-5 已定稿，**不得再改回 feature 方案**。
2. 仓库结构、构建命令、CI 步骤发生变化时**同步更新本文件**——这是唯一描述「怎么编译」的文档，过期即误导。
3. 影响构建/使用方式的变更须写入 `CHANGELOG.md [未发布]`（AGENTS.md 版本维护义务 A）。
4. 本文件不得出现私有仓库的实现细节（仅可提及仓库名与目录位置）。

---

## 13. 关联文档

- 商业决策与功能清单：[`00-monetization-overview.md`](./00-monetization-overview.md)
- 工程改造任务表（阶段 Q/R/S/T/U）：[`01-engineering-plan.md`](./01-engineering-plan.md)
- 目录结构约定：[`../../First/10-project-structure.md`](../../First/10-project-structure.md)
- 版本号单一来源：[`../version-management/01-versioning-policy.md`](../version-management/01-versioning-policy.md)
- 现有 CI/Release：[`ci.yml`](../../../.github/workflows/ci.yml) / [`release.yml`](../../../.github/workflows/release.yml)
- 进度真相源：[`../../First/12-plan.md`](../../First/12-plan.md)
