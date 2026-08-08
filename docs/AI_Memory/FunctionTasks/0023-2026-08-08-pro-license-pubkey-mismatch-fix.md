<!-- FT-0023 -->
# Pro 激活码验签失败（公钥不一致）+ license 脚本路径修复实录（2026-08-08）

> 场景：真机实测发现设备指纹 `IL5OBPZCJF` 的激活码无法激活；同时 `issue.py` 报 `FileNotFoundError: ...\SoundLink\soundlink-pro\license\vendor_sk.hex`。

## 根因分析（两个独立问题）

### 问题 1：脚本默认私钥路径指向旧布局目录名

- 双仓库重构（`2a5401b`，2026-08-07）把本地目录从 `SoundLink/` + `soundlink-pro/` 改为 `SoundLink/{oss,pro}/`，但 `issue.py`/`keygen.py` 的默认路径仍按旧目录名 `soundlink-pro` 拼接（`keygen.py` 甚至一直如此），且不命中时直接抛 traceback。
- 同会话此前已把 `issue.py` 目录名改为 `pro`（`1b85871`），但本次补全了鲁棒性：候选路径探测 + 缺文件时的明确报错。

### 问题 2：客户端内置验签公钥 ≠ 私仓权威私钥对应公钥（激活失败的真正原因）

时间线：

1. 2026-08-06 MON-01 落地（`f7e532d`）：`keygen.py` 生成**临时密钥对**，公钥 `qJB6…` 填入 `token.rs` `PUBKEYS_VENDOR_B64`，私钥仅存"仓库外"，未纳入任何版本管理。
2. 2026-08-07 私仓 `181597f`：把**另一把**私钥（公钥 `wKpx…`）写死进 `pro/license/vendor_sk.hex` 作为唯一权威来源；同日的重构更新了 `issue.py` 自检期望值，但 **`token.rs` 被漏改**，仍留着 `qJB6…`。
3. 结果：签发端（私仓私钥 → `wKpx…`）自检通过、正常出码；客户端（内置 `qJB6…`）验签全部失败，`Invalid("签名校验失败")`——所有已签发激活码无一能激活。

关键事实核查：

- `pro/license/vendor_sk.hex` 实地推导公钥 = `wKpxUUe0XZsacDcV2sAKXU9K7wGCiQxUk369M6PJvqU=`，与私仓 README 一致。
- `qJB6…` 的私钥不存在于任何仓库（工作区全局 Glob 仅一份 `vendor_sk.hex`）。
- 已发布 tag `v0.1.0-beta.1`（2026-08-04）**不含** `f7e532d`，即 `qJB6…` 从未随发布版流出 → C1「一经发布永不删除」不适用 → 采用**替换**而非追加。
- 台账 `license_ledger.csv` 中两笔 `IL5OBPZCJF` 签发均为私仓私钥所签，修正客户端后可直接验过，**无需重签**。

## 实现清单

| 文件 | 改动 |
|---|---|
| [token.rs](../../../desktop/src-tauri/src/license/token.rs) | `PUBKEYS_VENDOR_B64` 公钥 `qJB6…` → `wKpx…`（替换），注释更正为私仓权威来源 |
| [issue.py](../../../scripts/license/issue.py) | 默认私钥路径改为工作区根候选探测（`pro/` 优先、`soundlink-pro/` 回退）；缺私钥时明确报错（exit 2）替代 traceback；docstring/注释同步新布局 |
| [keygen.py](../../../scripts/license/keygen.py) | `--out` 默认值同样改候选探测，已有私钥处优先复用（防再次密钥分裂） |
| [10-license-management.md](../../user/10-license-management.md) | 三处旧目录名 `soundlink-pro` → `pro` |
| [CHANGELOG.md](../../../CHANGELOG.md) | `[未发布]` 修复小节回填两条 |

## 关键设计决策

- **替换而非追加公钥**：C1 只保护"已发布"的公钥；`qJB6…` 私钥已失、从未发布、从未签出任何台账记录中的 key，保留只会误导。若日后发现当时确有用它签发的 key，再按轮换流程把 `qJB6…` 追加回数组即可。
- **`keygen.py` 一并修复**：它不在用户报障范围内，但同样的旧路径会在新环境把私钥生成到错误位置——正是本次"双密钥"事故的同类隐患，属明确必要的同根因修复。

## 验证结果

- `roundtrip_check.py`：OK（签发端与 fixture 逐字一致，未被破坏）。
- `keygen.py`（不带参）：默认路径命中 `SoundLink\pro\license\vendor_sk.hex`，幂等复用并打印 `wKpx…`。
- `issue.py --sub IL5OBPZCJF --bind fingerprint --seats 3 --note AFD20260807-001`（不带 `--key`）：签发成功，台账追加正常。
- 临时脚本严格模拟 Rust `validate_token` 全流程（去空白大写 → 分段 → 无填充 base32 解码 → `checkvalid` 验签 → JSON 字段/版本/SKU/吊销/指纹比对）：
  - 旧公钥 `qJB6…`：`Invalid(签名校验失败)`——精确复现真机故障；
  - 新公钥 `wKpx…`：`Active(sub=IL5OBPZCJF, bind=fingerprint, seats=3)`。
- `cargo test --lib license::`：38 passed / 0 failed（含 `python_fixture_license_validates` 跨语言闭环）。
- `cargo clippy --lib`：零警告。

## 用户需自行完成部分

- **重新构建桌面客户端**（旧二进制内置的是 `qJB6…`，必须重编才会内置 `wKpx…`），然后用新签发的激活码在真机激活验证。
- 真机上已粘贴过的旧激活码可继续使用同一份文本（签名私钥未变），也可重新签发。

## 已知边界

- 若 2026-08-06~08-07 之间曾用临时私钥（`qJB6…` 对应）在台账之外签过 key，那些 key 在修正后客户端上会失效——目前无证据表明存在这种 key。
- 旧布局 `soundlink-pro/` 目录仅作存在性回退；两布局同时存在私钥时优先 `pro/`。

## 关键文件索引

- 验签：`desktop/src-tauri/src/license/token.rs`（`PUBKEYS_VENDOR_B64` / `validate_token`）
- 签发：`scripts/license/issue.py`、`keygen.py`、`roundtrip_check.py`、`ed25519_pure.py`
- 私仓：`pro/license/vendor_sk.hex`（权威私钥）、`README.md`（铁律）、`license_ledger.csv`（台账）
- 管理文档：`docs/user/10-license-management.md`

## 关联文档

- [FT-0021](./0021-2026-08-06-open-core-pro-implementation.md)（open-core/授权底座落地，本事故的引入会话）
- [FT-0020](./0020-2026-08-06-monetization-plan.md)（MON-01 方案，C1 约束来源）

## 建议版本级别

**不升版本**。license 功能尚未随任何版本发布（beta.1 不含），修复的是未发布功能的内部缺陷；按义务 A 已回填 CHANGELOG `[未发布]`，待发版时随该版本整体生效。
