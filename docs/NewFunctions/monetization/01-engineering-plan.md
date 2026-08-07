<!-- MON-01 -->
# Pro 授权与门控 · 完整工程改造方案

> 建档：2026-08-06 · 修订：2026-08-06（重新梳理：私有 Pro crate + 收窄至流转体验功能）
> 商业决策与功能清单见 [`00-monetization-overview.md`](./00-monetization-overview.md)

---

## 0. 工程红线

| # | 红线 | 理由 |
|---|---|---|
| E1 | **授权校验失败必须降级为免费版，绝不阻止启动或中断音频** | 付费用户被自己的软件锁死是最严重的信任事故 |
| E2 | **不联网、不上报任何信息** | 与 [`docs/privacy.md`](../../privacy.md) 零遥测承诺、CSP `connect-src 'self'` 强绑定 |
| E3 | **免费核心必须是完整可编译、可用、无残缺的产品** | 公开仓库默认构建产物即完整免费版；不留「此处省略」式空洞 |
| E4 | **门控只在一处判定** | 单一 `Entitlement` 真相源 + `soundlink-pro-api` trait 边界，禁止 `if is_pro` 散落各处 |
| E5 | **免费路径零额外开销** | 门控判定不进音频热路径；免费实现为编译期 no-op |
| E6 | **配置向下兼容** | 新增配置字段全部 `#[serde(default)]` |
| E7 | **Pro 源码不得出现在公开仓库任何提交中** | 包括注释、测试 fixture、CI 日志 |
| E8 | **已签发的 license 必须在所有后续版本中永久有效** | 「一次买断，含后续版本」是明示承诺。校验逻辑只能**放宽不能收紧**，见 §4.2 |

---

## 1. 现状基线（已读代码，2026-08-06）

| 关注点 | 现状 | 位置 |
|---|---|---|
| 三项自动化开关 | **已实装**：`AppConfig.auto_start` / `auto_receive_on_start` / `auto_send_on_start`；`get_app_settings` / `set_app_settings` 命令；`set_app_settings` 内同步 autostart 插件注册项 | [`commands/mod.rs`](../../../desktop/src-tauri/src/commands/mod.rs#L930-L1037) |
| 自动收发触发点 | **前端驱动**：`App.tsx` mount 时读 `get_app_settings`，按开关调 `start_receiver` / `connect_trusted_receiver` | [`App.tsx`](../../../desktop/ui/src/App.tsx#L316-L360) |
| 自动发送目标选择 | 当前取 `list_trusted_receivers` 的**第一个**有 host/port 的条目，无「上次设备」概念 | [`App.tsx`](../../../desktop/ui/src/App.tsx#L340-L351) |
| 信任存储 | `TrustStore` JSON 文件，`TrustedDevice` 含 `device_id` / `identity_pub_b64` / `name` / `last_seen` / `host` / `control_port` / `audio_port`；**无数量上限** | [`trust_store.rs`](../../../desktop/src-tauri/src/pairing/trust_store.rs#L36-L100) |
| 全局快捷键 | `main.rs` 内**无条件注册** `Ctrl+Shift+P`（切角色）/ `Ctrl+Shift+S`（显窗口），emit `global-shortcut` 事件 | [`main.rs`](../../../desktop/src-tauri/src/main.rs#L50-L77) |
| 托盘 | `commands::tray::setup_tray`；`close_action` 控制关窗行为 | [`main.rs`](../../../desktop/src-tauri/src/main.rs#L67) |
| 断线重连 | 发送端已有 `start_with_reconnect`；但**无「记住上次设备并在启动时重连」** | [`sender.rs`](../../../desktop/src-tauri/src/sender.rs#L229) |
| Ed25519 / keyring | 均已是直接依赖并在用（设备身份签名、私钥与固定配对码存储） | [`Cargo.toml`](../../../desktop/src-tauri/Cargo.toml)、[`device_identity.rs`](../../../desktop/src-tauri/src/device/device_identity.rs) |
| 稳定设备标识 | `device_id`（`device_id.txt` 明文公开标识） | [`device_identity.rs`](../../../desktop/src-tauri/src/device/device_identity.rs) |
| feature 体系 | `default = []`、`opus`、`wasapi`、`tauri_app`（聚合） | [`Cargo.toml`](../../../desktop/src-tauri/Cargo.toml) |

**关键判断**：
1. 验签所需依赖（`ed25519-dalek` / `base64` / `sha2` / `serde_json` / `keyring`）**全部已在**，无需新增第三方依赖。
2. PRO-1（自动化开关）**已有实现**，本次是把它们移到 Pro 侧 + 补「窗口不弹出」的完整体验。
3. PRO-2（自动重连上次设备）与 PRO-4（配置档）需新开发；PRO-3（设备记忆上限）只需在 `TrustStore` 加约束。
4. 现有自动收发逻辑在**前端**，Pro 门控若只放前端可被绕过 → **必须下沉到 Rust 侧**（见 R2）。

---

## 2. 仓库切分方案（private Pro crate）

> **构建与使用的完整操作说明见 [`02-multi-repo-guide.md`](./02-multi-repo-guide.md)**；本节只给结构与理由。

### 2.1 目标形态

```
公开仓库 SoundLink（MIT）
  desktop/pro-api/        ← 新增 crate soundlink-pro-api：只有 trait 与类型
  desktop/pro/            ← 新增 crate soundlink-pro：免费实现 FreeCapabilities
  desktop/src-tauri/
    src/license/          ← 新增：验签与指纹（公开、MIT）
    Cargo.toml            soundlink-pro-api = { path = "../pro-api" }
                          soundlink-pro     = { path = "../pro" }   ← 恒定 path 依赖

私有仓库 soundlink-pro（闭源，crate 名与公开侧同名）
  src/lib.rs              impl ProCapabilities for ProImpl（PRO-1~PRO-5 真实逻辑）
```

**切换机制：替换 `desktop/pro/` 目录内容**，而非 Cargo feature。

理由：Cargo 会解析**可选依赖**并写入 `Cargo.lock`，与 feature 是否启用无关；公开仓库一旦写入私有 git 依赖，无权限者连默认 `cargo build` 都会失败，直接违反 E3。目录替换让公开仓库的依赖图里永远只有公开 crate。
> 该 Cargo 行为**已于 2026-08-06 实测确认**（cargo 1.96.1，见 `02` 文档 §11 V-1）：未启用的可选 git 依赖仍被解析并导致默认构建失败。**目录替换方案定稿，不保留 feature 回退路径。**
> 实测同时给出两条硬约束：① 两份 `soundlink-pro` 的 `version` 必须一致（否则官方构建无法 `--locked`）；② 每次替换目录后必须 `cargo clean -p soundlink-pro`，否则 Cargo 增量缓存会**静默**沿用上次实现。详见 `02` §3.3 / §5.1 与 G10/G11。

### 2.2 为什么 trait 边界放公开侧 + 独立成 crate

- 公开侧需要**调用**这些能力，trait 必须可见。
- trait 只描述「能做什么」，不含「怎么做」。免费实现是**真实且合理的降级行为**（如 `max_remembered_devices() -> 1`、`startup_plan() -> None`），不是空洞占位（E3）。
- 这样公开仓库读起来是一个自洽的完整程序：「免费版就是设备记 1 台、不自动启动」。这是诚实的表述，不会被指责为诱饵。
- **必须独立成 crate**：`soundlink` 依赖 `soundlink-pro`，而 `soundlink-pro` 又需要 trait 定义；trait 若留在 `soundlink` 内会形成循环依赖。依赖方向须为 `soundlink → soundlink-pro → soundlink-pro-api`。

### 2.3 trait 草案

```rust
// soundlink-pro-api（公开 crate，MIT）
pub enum Entitlement { Free, Pro }

/// Pro 能力边界。免费实现返回受限行为，Pro 实现来自私有 crate。
pub trait ProCapabilities: Send + Sync {
    /// 可记忆的对端设备上限。
    fn max_remembered_devices(&self) -> usize;
    /// 启动时自动进入的模式（None = 不自动）。
    fn startup_plan(&self, cfg: &AppConfig) -> Option<StartupPlan>;
    /// 断线后是否自动重连、退避策略。
    fn reconnect_policy(&self) -> Option<ReconnectPolicy>;
    /// 配置档能力（None = 不支持多档）。
    fn profiles(&self) -> Option<&dyn ProfileStore>;
    /// 需注册的全局快捷键与其动作。
    fn shortcuts(&self) -> Vec<(String, ShortcutAction)>;
    /// 托盘直控菜单项。
    fn tray_items(&self) -> Vec<TrayItem>;
}
```

> 关键设计：**没有一个方法叫 `is_pro()`**。所有 Pro 差异都表达为「能力参数」，业务代码只按能力值行事，天然满足 E4/E5。

### 2.4 构建矩阵

| 场景 | `desktop/pro/` 内容 | 命令 | 结果 |
|---|---|---|---|
| 社区 / 默认 | 公开免费实现（仓库自带） | `cargo build --features tauri_app` | 免费版，完整可用 |
| 官方发布 | 私有实现（检出覆盖） | 同上 | Pro-capable 版，未激活时行为等同免费 |

**同一条命令，产物由目录内容决定。** 操作步骤见 [`02-multi-repo-guide.md`](./02-multi-repo-guide.md) §3。

---

## 3. 阶段 Q · 仓库切分与能力抽象（对应 M-B）

- [x] **Q1 · 新增 `soundlink-pro-api` crate** — `desktop/pro-api/`（公开、MIT）
  - 定义 `Entitlement` / `ProCapabilities` / `StartupPlan` / `ReconnectPolicy` / `ShortcutAction` / `TrayItem` / `ProfileStore`
  - **只有 trait 与数据类型，不含任何业务逻辑**
  - 依赖方向：`soundlink → soundlink-pro → soundlink-pro-api`（不可逆）
  - 验证：`cargo test -p soundlink-pro-api` 通过；该 crate 不依赖 `soundlink`
- [x] **Q2 · 新增 `soundlink-pro` crate（免费实现）** — `desktop/pro/`（公开、MIT）
  - `FreeCapabilities`：`max_remembered_devices() = 1`、`startup_plan() = None`、`reconnect_policy() = None`、`profiles() = None`、`shortcuts()` 仅返回 `Ctrl+Shift+S`（显窗口，免费保留）、`tray_items()` 仅返回基础项
  - 导出 `pub const EDITION: &str = "community";` 与工厂 `pub fn capabilities() -> Arc<dyn ProCapabilities>`
  - [`Cargo.toml`](../../../desktop/src-tauri/Cargo.toml) 加**恒定 path 依赖**（不加 `pro` feature、不加 optional）
  - 版本号写定后**不再变动**，且私有侧必须一致（`02` §5.1 / G11）
  - 验证：`cargo test -p soundlink-pro` 覆盖免费实现各返回值；`cargo build --features tauri_app` 通过
- [x] **Q3 · 私有仓库 `soundlink-pro` 骨架** — crate 名与公开侧**同名、同版本号**
  - 依赖 `soundlink-pro-api` 用**相对路径** `../pro-api`（挂载后解析到公开仓库；禁用绝对路径，会触发 lockfile collision，见 `02` §5.1）；导出 `EDITION = "official"` 与同名工厂函数
  - 首版可只做 `max_remembered_devices() = 8`，验证整条替换通路
  - 验证：`desktop/pro/` 替换为私有实现后 `cargo build --features tauri_app` 通过，且免费实现恢复后同样通过；**每次替换后必须 `cargo clean -p soundlink-pro`**（V-4 已证实不清理会构建出错版本产物且无报错）
- [x] **Q4 · `AppState` 挂载能力对象** — [`commands/mod.rs`](../../../desktop/src-tauri/src/commands/mod.rs#L43)
  - 加 `pub caps: Arc<dyn ProCapabilities>`、`pub entitlement: Arc<RwLock<Entitlement>>`
  - `AppState::new` 中构造；免费构建下恒为 `Free`
  - 验证：现有 42 个命令行为逐项不变
- [x] **Q5 · CI 双构建** — `.github/workflows/`
  - 公开 CI：用仓库自带免费实现（无任何 secret），确保社区 fork 可通过
  - 发布 CI：仅 tag 触发；用只读细粒度 token 把私有实现检出覆盖 `desktop/pro/` 后构建；**日志不得打印私有仓库内容或 token**（E7）
  - 检出后**必须 `cargo clean -p soundlink-pro`**（`rust-cache` 恢复的 `target/` 会串味）；构建后不做任何 git 写操作（避免私有依赖清单回流，见 `02` §5.2）
  - 完成 `02` §11 的 **V-6**（Tauri NSIS 打包在替换后的表现）与 **V-7**（junction 下 clippy）
  - 验证：两条流水线各跑通一次；fork 上的 PR 公开 CI 全绿

**阶段验收**：`cargo test` + `cargo clippy --features tauri_app -- -D warnings` 全绿；免费构建功能回归无变化。

---

## 4. 阶段 R · 授权底座（对应 M-C）

### 4.1 License 规格

**格式**：`SLPRO-<base32(payload_json)>-<base32(ed25519_sig)>`（校验时忽略 `-` 与空白、统一大写）

**Payload 字段**：

| 字段 | 类型 | 说明 |
|---|---|---|
| `v` | u8 | 格式版本，当前 `1` |
| `sku` | string | `"desktop-pro"` |
| `iat` | u64 | 签发时间（Unix 秒） |
| `exp` | u64? | 过期时间；买断留空表示永久 |
| `sub` | string | 买家标识：设备指纹（方案 A）或订单号哈希（方案 B） |
| `bind` | string | `"fingerprint"` / `"order"` |
| `seats` | u8 | 允许设备数，默认 3 |
| `nonce` | string | 8 字节随机 base32，用于吊销与泄露溯源 |

**签名**：`Ed25519(sk_vendor, canonical_json_bytes(payload))`；客户端内置 `PUBKEY_VENDOR_B64`。

**校验链**（全程离线）：
```
读 license（keyring → license.key 文件兜底）
 → 规范化 → 拆段 → 校验前缀 → base32 解码
 → Ed25519 verify                      失败 → Invalid
 → 解析 payload，v 不高于支持版本       失败 → Invalid
 → exp 非空且过期                            → Expired
 → nonce 命中内置吊销名单                    → Revoked
 → bind=fingerprint 时比对本机指纹      不符 → DeviceMismatch
 → Active
```
**任何非 `Active` 一律等价免费版**（E1），只在设置页展示原因，不弹阻塞对话框。

**设备指纹**：`base32(sha256("soundlink-fp-v1" || machine_id || device_id))[..10]`
- `machine_id`：Windows 读 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`；macOS `IOPlatformUUID`；Linux `/etc/machine-id`；**取不到即回退纯 `device_id`，不报错**
- 单向哈希、无隐私信息，UI 旁须写明

**存储**：license 文本存 keyring（`service="soundlink"`, `account="pro_license"`），兜底 `<config_dir>/license.key`。
> 与 `fixed_pairing_code` 不同，license **允许明文文件兜底**：它不是用户的安全凭据，泄露只影响作者收入。此差异须在代码注释写明，避免后来者误判为安全缺陷。

### 4.2 跨版本兼容：软件更新后买断如何自动继续有效

**目标**：用户装 `v0.3.0` 激活一次，升到 `v0.9.0` / `v1.4.0` 后**无需任何操作**仍是 Pro。

#### 为什么默认就能做到

license 中**不含版本号**，校验也**不比较软件版本**。payload 只有 `sku` / `iat` / `exp` / `sub` / `bind` / `seats` / `nonce`（§4.1），因此「用哪个版本的 SoundLink 打开」不影响结论。

存储位置在**用户配置域而非安装目录**：keyring（`service="soundlink"`, `account="pro_license"`）+ 兜底 `%APPDATA%\soundlink\license.key`（与 `app_config.json` / `device_id.txt` 同目录，见 [`main.rs`](../../../desktop/src-tauri/src/main.rs#L165-L171)、[`config/mod.rs`](../../../desktop/src-tauri/src/config/mod.rs#L136)）。NSIS 升级安装与免安装 exe 覆盖都**不触碰**该目录，所以升级后 license 原地可读。

> `identifier = "com.soundlink.desktop"`（[`tauri.conf.json`](../../../desktop/src-tauri/tauri.conf.json#L5)）与 keyring service 名均已固定，二者**永不可改**——改了等于所有存量 license 失效。

#### 必须遵守的 5 条兼容约束

| # | 约束 | 违反后果 |
|---|---|---|
| **C1** | `PUBKEY_VENDOR_B64` 一经发布**永不删除**。若将来轮换密钥，改为**公钥数组** `PUBKEYS_VENDOR: &[&str]`，新公钥追加、旧公钥保留，验签**任一命中即通过** | 删除旧公钥 = 所有存量 license 一夜失效 |
| **C2** | payload 版本判定用 `v <= LICENSE_FORMAT_MAX`（**向后**兼容旧格式），不得写 `v == 1` | 老 license 在新版本被判 `Invalid` |
| **C3** | 指纹算法**带版本前缀且并行保留**：现为 `soundlink-fp-v1`。若算法必须变更，新版本同时计算 v1 与 v2，**任一匹配即通过**（`fingerprint_candidates() -> Vec<String>`） | 换算法 = 所有 `bind=fingerprint` 的 license 全部 `DeviceMismatch` |
| **C4** | `sku` 白名单**只增不减**；`"desktop-pro"` 永久在列 | 重命名 SKU = 存量失效 |
| **C5** | 新增 payload 字段一律 `#[serde(default)]` 且旧 license 缺失时取「宽松默认」（如未来加 `tier` 字段，缺失视为最高档而非最低档） | 旧 license 被新字段判定为低档 |

> 归纳为一条判据：**任何 license 校验相关改动，必须先问「已发出的 key 在改动后还能通过吗」；答案非「能」则不做**（E8）。

#### Pro 功能面在版本演进中的扩张

「含后续 Pro 新功能」意味着**未来新增的 Pro 能力不得要求重新购买**。落地方式即 `ProCapabilities` trait 的设计（§2.3）：新增 Pro 能力是给 trait **加方法**，Pro 实现返回真实能力、免费实现返回受限值。license 侧**不感知**能力清单，故不需要新 key。

因此 **不引入分档 SKU**（§9 不做清单）。一旦出现 `Pro` / `Pro+`，就必须在 license 内表达档位，C5 的兼容负担与用户困惑都会显著上升。

#### 保留但暂不启用的字段

`exp`（过期时间）在买断模型下**签发时一律留空**。它存在的唯一目的是：若将来出现「限时授权」需求（如媒体评测 key），校验代码已就绪，无需改格式版本。**买断 key 永不写 `exp`**。

#### 系统时间与更新的交互

`exp` 为空时校验**完全不读系统时钟**，所以改系统时间不影响买断 key（U6 已覆盖）。这也意味着「升级后因时间同步导致失效」的风险不存在。

#### 更新提示机制的边界

当前项目**未接入 `tauri-plugin-updater`**（已核查：无任何 updater 配置与依赖），升级靠用户从 Release 页手动下载。这对本方案是**有利的**：无需处理「更新通道是否区分免费/Pro」的问题。

若将来接入自动更新，须遵守：
- **更新检查与分发对免费/Pro 一视同仁**，不做「Pro 优先更新」（00 文档 §6 已排除支持优先级差异）。
- 更新元数据源不得携带 license 信息（E2）。
- 官方发布线**只有一种产物**（Pro-capable，未激活时行为等同免费版），因此更新不存在「Pro 用户被更新成免费版」的风险。这一点由 [`02-multi-repo-guide.md`](./02-multi-repo-guide.md) §6 的分发决策保证——**该决策不可改，否则自动更新必须先解决产物区分问题**。

#### 对应任务

- [x] **R8 · 公钥数组化与兼容判定** — `license/token.rs` 直接实现为 `PUBKEYS_VENDOR: &[&str]`（首发仅一项）+ `LICENSE_FORMAT_MAX` + `sku` 白名单常量
  - 一开始就做成可扩张形态，避免日后改结构时破坏兼容
  - 验证：单测「旧格式 v1 license 在 `LICENSE_FORMAT_MAX=2` 下仍通过」、「第二个公钥签发的 key 也通过」
- [x] **R9 · 指纹候选集** — `license/fingerprint.rs` 提供 `fingerprint_candidates() -> Vec<String>`（首发仅 v1），比对时任一命中即通过
  - 验证：单测断言候选集含 v1，且比对使用 `contains` 语义
- [x] **R10 · 升级保持测试** — 模拟「旧版本写入 license → 新版本读取」
  - 覆盖：keyring 路径 + 文件兜底路径；`v` 值低于当前上限；payload 含未知多余字段（须被忽略而非报错）
  - 归入 U1 计数

### 4.3 进度表

- [x] **R1 · `license` 模块** — `desktop/src-tauri/src/license/{mod.rs,token.rs,fingerprint.rs,revocation.rs}`
  - `LicenseState`：`Free` / `Active{sub,iat,seats}` / `Invalid(reason)` / `Expired` / `Revoked` / `DeviceMismatch`
  - base32 编解码自实现（约 40 行），**不加新 crate**
  - 验证：`cargo test license::` ≥ 12 例（见 U1）
- [x] **R2 · vendor 公钥常量** — `license/token.rs` 内 `PUBKEY_VENDOR_B64`
  - 私钥绝不入库；单测断言解码长度 32 + 一份 committed 测试 fixture license
- [x] **R3 · 设备指纹** — `license/fingerprint.rs`
  - Windows 注册表读取需 `windows` crate 加 `Win32_System_Registry` feature
  - 验证：同机两次结果一致；单测覆盖 `machine_id` 缺失回退分支
- [x] **R4 · Entitlement 注入** — `AppState::new` 中加载并验签一次，写入 `entitlement`
  - **`Free` 是正常状态**：加载失败仅 `tracing::info!`，不 warn 不 error
  - `pro` feature 未启用时**跳过整个校验流程**（免费构建无需 license 代码路径参与）
  - 验证：无 license 启动日志无告警
- [x] **R5 · Tauri 命令三件套** — `commands/mod.rs` + [`main.rs`](../../../desktop/src-tauri/src/main.rs#L94) 注册
  - `get_license_status() -> LicenseInfo`（`entitlement` / `state` / `sub_masked` / `fingerprint` / `pro_build: bool`）
  - `activate_license(key)`（验签通过则写 keyring + 更新 entitlement + emit `license-changed`）
  - `deactivate_license()`（清 keyring 与文件，回落 Free；给用户「换机前先释放」的确定性）
  - `sub_masked` 只回显前 4 后 2 字符，避免截图泄露
  - **免费构建下 `pro_build = false`**，前端据此把 Pro 区块显示为「本构建不含 Pro（社区版）」而非「点击购买」
  - 验证：粘贴激活 → 状态变 Pro → 重启仍 Pro → 反激活回 Free
- [x] **R6 · 吊销名单** — `license/revocation.rs` 静态 `REVOKED_NONCES: &[&str]`，首发空数组
  - 验证：临时插入测试 nonce 的单测
- [x] **R7 · 前端 Pro 区块** — [`SettingsPanel.tsx`](../../../desktop/ui/src/components/SettingsPanel.tsx) 新增「授权」`section`
  - 展示：当前状态 / 设备指纹（一键复制）/ 激活输入框 / 购买链接（走既有 `openExternal`）/ 反激活
  - listen `license-changed` 即时刷新，无需重启
  - 验证：三种状态（社区构建 / Pro 未激活 / Pro 已激活）UI 均正确

**阶段验收**：U1、U2 通过；免费构建行为回归无变化。

---

## 5. 阶段 S · Pro 功能实装（对应 M-D）

> 每项都遵循同一骨架：**能力值来自 `caps`，业务代码不判断 is_pro**（E4）。

### S-A · PRO-3 设备记忆上限（先做，最简单且被其他项依赖）

- [x] **S1 · `TrustStore` 容量约束** — [`trust_store.rs`](../../../desktop/src-tauri/src/pairing/trust_store.rs#L80)
  - `add()` 增加 `max: usize` 参数（或 `TrustStore` 构造时注入上限）
  - 超限行为：**替换 `last_seen` 最旧的条目**，而非拒绝新配对
    - 理由：拒绝会让免费用户在「换手机」时卡死，体验极差且会招致差评；替换最旧的符合「记忆 1 台 = 记住最近用的那台」的直觉
  - 上限来源：`caps.max_remembered_devices()`（免费 1 / Pro 8）
  - 被替换时 emit 事件 → 前端提示「已替换最久未用的设备（免费版可记忆 1 台）」
  - 验证：免费实现下第 2 次配对替换第 1 条；Pro 实现下累积到 8 后才替换；单测覆盖两种上限
- [x] **S2 · UI 提示** — 设备列表旁标注 `1/1`（免费）或 `3/8`（Pro）
  - 验证：数量随配对变化正确

### S-B · PRO-1 开机自启 + 启动即进入收/发模式

- [x] **S3 · 自动收发逻辑下沉到 Rust** — 现状在 [`App.tsx`](../../../desktop/ui/src/App.tsx#L316-L360)，前端门控可被绕过
  - 新增命令 `resolve_startup_plan() -> Option<StartupPlan>`，内部走 `caps.startup_plan(&cfg)`
  - 前端只负责「拿到 plan 就执行对应现有命令 + 更新 UI 状态」，不再自行读开关判断
  - 免费实现恒返回 `None` → 前端自然不执行任何自动启动
  - 验证：免费构建下即使手工把 `app_config.json` 的 `auto_receive_on_start` 改为 `true`，**也不会自动启动**（这是门控有效性的关键验收项）
- [x] **S4 · `set_app_settings` 门控** — [`commands/mod.rs`](../../../desktop/src-tauri/src/commands/mod.rs#L987)
  - `auto_start` / `auto_receive_on_start` / `auto_send_on_start` 三参数在免费下**忽略并返回当前值**（不报错、不写入）
  - 返回结构加 `automation_available: bool` 供前端置灰
  - 保留 `close_action` / `onboarding_completed` / `sender_drm_hint_seen` 免费可写
  - 验证：免费下调 `set_app_settings` 传 `auto_start=true`，返回仍为 `false` 且 autostart 注册项未创建
- [x] **S5 · 静默启动（窗口不弹出）** — Pro 体验的关键差异
  - `--autostarted` 参数（[`main.rs`](../../../desktop/src-tauri/src/main.rs#L47) 已传入）时，Pro 下**直接最小化到托盘**，不显示主窗口
  - `tauri.conf.json` 主窗口保持 `visible: true`，改为运行时按参数 `hide()`（避免免费版行为改变）
  - 验证：开机后无窗口弹出，托盘图标显示「接收中」，音频可用
- [x] **S6 · 前端设置页门控 UI** — [`SettingsPanel.tsx`](../../../desktop/ui/src/components/SettingsPanel.tsx)
  - 三个开关在免费下置灰 + 「Pro」徽标 + 一行说明 + 「了解 Pro」链接
  - 验证：激活后即时可用（listen `license-changed`）

### S-C · PRO-2 记忆并自动重连上次设备

- [x] **S7 · 「上次设备」持久化** — [`config/mod.rs`](../../../desktop/src-tauri/src/config/mod.rs)
  - 加 `#[serde(default)] pub last_peer_device_id: Option<String>`（区别于已有 `last_receiver_addr`，后者只是地址无身份）
  - 成功建立连接时写入（接收端记发送端、发送端记接收端）
  - **此字段免费版也写入**（记录行为无害），只是免费不消费它
  - 验证：连接后重启，配置文件含正确 device_id
- [x] **S8 · 自动重连策略** — `caps.reconnect_policy()`
  - Pro：启动时按 `last_peer_device_id` 查 `TrustStore` 直连；断线后指数退避重试（1s/2s/4s/8s/上限 30s），静默不弹窗
  - 免费：`None`，行为与现状一致（发送端既有 `start_with_reconnect` 的会话内重连**保持免费**——那是「本次连接的鲁棒性」，属流转本体，不能收费）
  - **边界明确**：Pro 卖的是「**跨启动**的自动重连」，不是「连接过程中的容错」
  - 验证：拔网线 → 恢复后自动恢复播放；免费构建下需手动重连
- [x] **S9 · 发送端目标选择修正** — 替换 [`App.tsx`](../../../desktop/ui/src/App.tsx#L340-L351) 的「取第一个」逻辑
  - 改为按 `last_peer_device_id` 优先，回退到 `last_seen` 最新
  - 验证：多台已信任接收端时连到正确的那台

### S-D · PRO-4 多套配置一键切换

- [x] **S10 · `Profile` 数据结构** — [`config/mod.rs`](../../../desktop/src-tauri/src/config/mod.rs)
  - `pub struct Profile { id, name, output_device: Option<usize>, jitter_mode: String, volume: f32, audio_params: AudioParams, role: String, peer_device_id: Option<String> }`
  - `AppConfig` 加 `#[serde(default)] pub profiles: Vec<Profile>`、`#[serde(default)] pub active_profile: Option<String>`
  - 上限 8；免费下 `caps.profiles()` 为 `None`，命令直接返回受限提示
  - 验证：老 `app_config.json` 可正常加载（E6）
- [x] **S11 · 命令** — `list_profiles` / `save_profile` / `apply_profile` / `delete_profile` / `rename_profile`
  - `apply_profile` **复用**既有 `select_output_device` / `set_jitter_mode` / `set_volume` / `set_audio_params` 内部逻辑，不重复实现
  - 涉及 `restart_required` 的参数变更返回提示，不静默重启流
  - 验证：切换后 `get_desktop_settings` 各字段与档内一致
- [x] **S12 · UI** — 新增 `desktop/ui/src/components/ProfilePanel.tsx`，挂设置页
  - 免费下显示 2 个示例档（灰色不可点）+ Pro 徽标
  - 验证：激活前后状态切换正确

### S-E · PRO-5 全局快捷键与托盘直控

- [x] **S13 · 快捷键注册改为能力驱动** — [`main.rs`](../../../desktop/src-tauri/src/main.rs#L50-L77)
  - 现状硬编码两个快捷键 → 改为遍历 `caps.shortcuts()`
  - 免费：仅 `Ctrl+Shift+S`（显示主窗口，属基本可用性，保持免费）
  - Pro：追加 `Ctrl+Shift+P`（切角色）、开始/停止收发、切换输出设备、静音切换
  - handler 中 `ShortcutAction` → emit 对应事件（沿用现有 `global-shortcut` 事件通道，`kind` 扩展）
  - 验证：免费下 `Ctrl+Shift+P` 无响应；Pro 下全部生效
- [x] **S14 · 快捷键自定义（Pro）** — `AppConfig` 加 `#[serde(default)] pub shortcuts: Vec<ShortcutBinding>`
  - 冲突检测：注册失败时提示而非静默忽略（现状仅 `tracing::warn!`）
  - 验证：改绑后重启仍生效；与系统占用冲突时有明确提示
- [x] **S15 · 托盘直控菜单** — `commands/tray.rs`
  - 菜单项来自 `caps.tray_items()`；免费仅「显示主窗口 / 退出」
  - Pro 追加「开始/停止接收」「开始/停止发送」「静音」「切换到配置档 →」子菜单
  - 菜单文字随状态更新（如「开始接收」↔「停止接收」）
  - 验证：不打开主窗口即可完成开始→停止全流程

**阶段验收**：U3–U5 通过。

---

## 6. 阶段 T · 签发工具链与销售落地（对应 M-E）

- [x] **T1 · 密钥生成脚本** — `scripts/license/keygen.py`（用项目根 `.venv`，禁系统 python）
  - 生成 vendor Ed25519 密钥对；私钥输出到**仓库外**路径；打印公钥 base64 供填 R2
  - `.gitignore` 加 `scripts/license/*.pem`、`*_sk*`、`license_ledger.csv`
  - 输出须醒目提示：**私钥丢失 = 无法再签发新 key（已发出的仍可用）**
- [x] **T2 · 签发脚本** — `scripts/license/issue.py`
  - 入参 `--sub <指纹|订单号> --bind fingerprint|order [--seats 3] [--note 订单号]`
  - 输出 license 文本 + 追加本地台账 `license_ledger.csv`（不入库）
- [x] **T3 · 跨语言一致性测试** — `scripts/license/roundtrip_check.py` + Rust fixture
  - Python 签发 → Rust 验签通过；Rust 拒绝篡改样本
  - 用 committed 测试密钥对，纳入公开 CI（不需真实私钥）
- [x] **T4 · 换机/重装 SOP** — 追加到 [`docs/user/08-troubleshooting.md`](../../user/08-troubleshooting.md)（**不新建 md**）
  - 旧订单号 + 新指纹 → 查台账 → 重签发；明确「免费重签不限次数」
- [ ] **T5 · 爱发电商品页** — 9.99 档位；含 00 文档 §7.3 四句话 + 指纹获取图示 + 「备注填指纹」
- [ ] **T6 · 淘宝小店** — 同上，旺旺索取指纹
- [ ] **T7 · 官网 Pro 页** — [`website/`](../../../website/) 新增 Pro section（复用 `SectionShell`；文案进 `content/zh.ts`/`en.ts`）
  - 内容 = 00 文档 §5 对照表 + §7.3 口径 + 购买按钮外链
  - **必须包含 open-core 说明**（核心开源可自编译，Pro 增强闭源）
  - 验证：`npm run build` 通过；中英双语无缺项
- [x] **T8 · README 修订** — [`README.md`](../../../README.md) / [`README.en.md`](../../../README.en.md)
  - 现有「MIT，完全免费」「无广告无订阅」表述需修订为准确的 open-core 描述
  - 新增「免费 vs Pro」小节（位置：「已知限制」之后）
  - 同步修订 [`01-market-research.md`](../opensource-launch/01-market-research.md) 中的自我定位措辞
- [x] **T9 · 隐私政策** — [`docs/privacy.md`](../../privacy.md) 补：license 校验完全离线、指纹为单向哈希不外传、无激活服务器

---

## 7. 阶段 U · 测试与质量门

- [x] **U1 · license 单测 ≥ 12 例** — 合法 / 篡改 payload / 篡改 sig / 错前缀 / 坏 base32 / 坏 JSON / 版本过高 / 已过期 / 永久无 exp / 指纹符 / 指纹不符 / 吊销命中
  - **全项目最不能出错的模块**（锁死付费用户风险，E1）
- [x] **U2 · 门控有效性测试** — 每项 Pro 能力各一例
  - 重点：**手工篡改 `app_config.json` 无法在免费构建下启用自动化**（S3 验收项）
  - 免费构建下 Pro 命令返回受限提示且无副作用
- [ ] **U3 · 免费版完整性回归** — 手动清单：配对 / Android→Win 收发 / Win→Win 收发 / 参数调整 / 音量 / 设备切换 / 托盘最小化 / 日志面板 / 首次引导
  - 确认免费版是**完整可用产品**（E3）
- [ ] **U4 · Pro 端到端** — 开机自启 → 静默进入接收 → 自动重连上次设备 → 快捷键停止 → 托盘恢复 → 切换配置档
- [ ] **U5 · 设备记忆边界** — 免费第 2 台替换第 1 台且有提示；Pro 累积至 8 后替换最旧
- [ ] **U6 · 降级路径演练** — 破坏 keyring 条目 / 改坏 license 文件 / 改系统时间到未来 → 确认**降级为免费且音频不中断**
- [ ] **U6b · 升级保持演练**（对应 §4.2 / R10）— 装旧版 → 激活 → 覆盖安装新版（NSIS 升级 + 免安装 exe 替换两种路径）→ 确认**仍为 Pro 且无需重新激活**
  - 同时确认配置目录未被安装器清理、keyring 条目未失效
- [ ] **U7 · 性能门** — 免费构建 CPU/内存与基线持平（±2%）；Pro 自动重连轮询不增加空闲 CPU（无连接时不应有忙等）
- [x] **U8 · 双构建 CI 绿** — 公开 CI（免费）+ 发布 CI（Pro）各通过；`cargo clippy -- -D warnings` + `npm run build` 全绿

---

## 8. 实施顺序

```
Q1(含 trait crate 拆分) → Q2 → Q3 → Q4 → Q5      仓库切分，必须先稳
R1 → R2 → R3 → R4 → R5 → R7                       授权底座
        ↘ R6                                       吊销名单
        ↘ R8 → R9 → R10                            跨版本兼容（与 R1/R3 同期做，事后补成本高）
S1 → S2                                            PRO-3 设备上限（最简，先验证门控通路）
S3 → S4 → S5 → S6                                  PRO-1 自动化（价值最高）
S7 → S8 → S9                                       PRO-2 自动重连（依赖 S7 字段）
S10 → S11 → S12                                    PRO-4 配置档
S13 → S14 → S15                                    PRO-5 快捷键与托盘
T1 → T2 → T3                                       签发链（可与 S 并行）
T4 → T9                                            销售与文档落地
U1–U8                                              贯穿
```

**关键路径**：`Q1..Q5 → R1..R7 → S1,S2 → S3..S6 → T1..T3 → 剩余 S → T4..T9 → U`。

**首个可卖版本的最小集**：Q 全部 + R 全部 + S1–S6（设备上限 + 自动化）+ T1–T3、T5、T7、T8。PRO-2/4/5 可在 `v0.3.1` 补齐，但**对照表需如实标注「即将推出」**，不得预先宣传为已有。

---

## 9. 不做清单

| 不做 | 理由 |
|---|---|
| 联网激活服务器 | E2 |
| 代码混淆 / 反调试 / 二进制自校验 | 私有 crate 已达成「编译不出来」的目标，额外对抗收益低、损害信任 |
| 限时试用与到期锁定 | 00 文档 P4 |
| 多档 SKU（Pro / Pro+） | 9.99 单价下分档只增复杂度 |
| 移动端授权校验 | 移动端全功能免费 |
| 把会话内断线重连收费 | S8 已明确：那属流转本体鲁棒性 |
| 超限时拒绝新配对 | S1 已明确：改为替换最旧条目 |

---

## 10. 回填规则（强约束）

1. 完成任务立即 `[ ]` → `[x]`，行末补 `— YYYY-MM-DD 备注`。
2. 阶段完成后更新 [`00-monetization-overview.md`](./00-monetization-overview.md) §11 里程碑表。
3. 用户可感知变更写入 `CHANGELOG.md [未发布]`；**禁止自行修改 `VERSION`**，清单 version 一律走 `scripts/sync_version.py`。
4. Pro 源码不得进入公开仓库（E7）；提交前自查 diff。
5. 验收未过不得标完成。
6. **任何 license 校验相关改动，须先自证「已签发的 key 改动后仍能通过」**（E8 / §4.2 C1–C5）；不能自证则不做。
7. 会话结束后在 `docs/AI_Memory/FunctionTasks/` 按 AGENTS.md 流程归档。

---

## 11. 关联文档

- 商业决策与功能清单：[`00-monetization-overview.md`](./00-monetization-overview.md)
- **多仓库构建与使用指南**：[`02-multi-repo-guide.md`](./02-multi-repo-guide.md)
- 安全模型（keyring 复用依据）：[`../../First/05-pairing-security.md`](../../First/05-pairing-security.md)
- 编码规格：[`../../First/11-implementation-spec.md`](../../First/11-implementation-spec.md)
- 延迟与体验（自动重连体验依据）：[`../../First/06-latency-experience.md`](../../First/06-latency-experience.md)
- 免费路线任务：[`../release-readiness/03-p2-future-optimizations.md`](../release-readiness/03-p2-future-optimizations.md)
- 版本递增判定：[`../version-management/01-versioning-policy.md`](../version-management/01-versioning-policy.md)
- 官网规划（T7 落点）：[`../opensource-launch/02-website-plan.md`](../opensource-launch/02-website-plan.md)
- 进度真相源：[`../../First/12-plan.md`](../../First/12-plan.md)
