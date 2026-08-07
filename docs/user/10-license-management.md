# 10 · Pro 激活码的生成与管理

> 面向：项目作者本人（签发/管理激活码）。终端用户的激活与换机流程见 [`08-troubleshooting.md`](./08-troubleshooting.md)「Pro 授权」一节。
> 技术规格见 [`../NewFunctions/monetization/01-engineering-plan.md`](../NewFunctions/monetization/01-engineering-plan.md) §4（MON-01 阶段 R/T）。
> 脚本统一用**项目根 `.venv`** 的 Python 运行（禁系统 python 装包）。

---

## 1. 模型速览

- **买断制、离线校验**：激活码（license）是一段 `SLPRO-…` 文本，内含 Ed25519 签名的授权载荷。客户端用**内置公钥**在本机离线验签，**不联网、不上报任何信息、无激活服务器**。
- **指纹绑定**：激活码绑定购买时提交的设备指纹（10 位单向哈希短码）。换机/重装系统指纹变化需**免费重签**（不限次数）。
- **一次买断，永久有效**：载荷不含软件版本号，校验不比对版本；`exp` 字段一律留空（永久）。已签发的 key 在所有后续版本中自动继续有效（红线 E8）。
- **同一授权最多 3 台设备**（`seats`，默认 3），7 日内可退。

载荷字段：`v`（格式版本）/ `sku`（`desktop-pro`）/ `iat`（签发时间）/ `exp`（过期，买断留空）/ `sub`（指纹或订单号）/ `bind`（`fingerprint`/`order`）/ `seats` / `nonce`（吊销与溯源）。

---

## 2. 一次性准备：生成 vendor 密钥对（只生成一次，终身不变）

激活码用 **Ed25519 私钥签发、公钥验签**。私钥是签发能力的唯一凭据。

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink\oss
.\.venv\Scripts\python.exe scripts\license\keygen.py
```

- 私钥写入**私仓** `D:\CodeProject\TRAE_Projects\SoundLink\pro\license\vendor_sk.hex`（`--out` 默认值即此；脚本强制校验必须位于公开仓库之外）。
- **已存在私钥时 `keygen.py` 直接复用、不再随机生成**（保证公钥固定）；仅显式加 `--force` 才重新生成（轮换，见 §5）。
- 公钥 base64 打印在终端，填入 [`desktop/src-tauri/src/license/token.rs`](../../desktop/src-tauri/src/license/token.rs) 的 `PUBKEYS_VENDOR_B64`（已在首次生成时填入）。
- 公开仓 `.gitignore` 已排除 `scripts/license/*.pem`、`*_sk*`（`test_sk.hex` 例外）、`license_ledger.csv`；真实私钥只在私仓 `soundlink-pro`。

### 2.1 核心铁律：`vendor_sk.hex` 与发布版本强绑定，生成后禁止更改

> 🔴 **`vendor_sk.hex` 一生只生成一次，与客户端内置公钥一一对应。任何"删除重生成 / 换机重生成 / 重拉代码后重生成"都会让新旧私钥不一致——新私钥签出的激活码，在已发布（内置旧公钥）的软件上验签必失败，表现为"所有用户都无法激活"。**

- 客户端 `PUBKEYS_VENDOR_B64` 是**编译期写死**的。软件发出后，只有"私钥 = 生成该公钥的那一把"签出的码才能通过校验。
- 因此 `vendor_sk.hex` **不是每次环境搭建时重新生成的临时物**，而是和某一代软件版本**永久绑定的资产**。

### 2.2 推荐做法：把 `vendor_sk.hex` 写死进私仓，随代码版本化管理

已落地：私钥写死在**私仓** `D:\CodeProject\TRAE_Projects\SoundLink\pro\license\`，并随该私有仓库版本管理：

1. 私仓 `soundlink-pro/license/` 内含 `vendor_sk.hex`（私钥）+ `license_ledger.csv`（台账）+ `README.md`（记录当前公钥 base64 与铁律）。
2. `issue.py` / `keygen.py` 的默认路径已指向该私仓目录——**直接运行脚本即可，无需传 `--key`/`--out`**。
3. 换机/新环境：克隆私仓 `soundlink-pro` 到 `..\soundlink-pro`（与公开仓同级），脚本默认路径自动命中同一份私钥，**绝不重新随机生成**。
4. 双保险：`issue.py` 签发前会**由私钥推导公钥并与内置期望值比对**，不一致立即中止（防止拿错/重生成私钥后误签无效码）。

> 这样无论换电脑、重装系统、重拉源码，签发用的私钥永远唯一，从源头杜绝"环境变了→私钥变了→全员无法激活"。

> ⚠️ "纳入私仓"指**私有的、仅本人可见的仓库**，绝不允许进入公开源码仓或任何他人可见的地方。公开仓的 `.gitignore` 排除规则保持不变。
> ⚠️ **私钥丢失 = 无法再签发新 key**（已发出的 key 仍永久有效，E8）。私仓之外再留一份离线备份（加密 U 盘 / 离线密码管理器）。
> ⚠️ **私钥泄露 = 任何人都能签发有效 key**。不要截图、不要发到聊天工具、不要提交到公开仓。

---

## 3. 签发激活码

用户下单时提交**设备指纹**（设置 → 授权 → 一键复制），卖家据此签发。

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink\oss
.\.venv\Scripts\python.exe scripts\license\issue.py `
    --sub <用户设备指纹> --bind fingerprint --seats 3 --note <订单号>
```

> 私钥默认从私仓 `soundlink-pro\license\vendor_sk.hex` 读取，无需 `--key`。

- 输出的 `SLPRO-…` 即激活码，通过爱发电私信 / 淘宝旺旺回发给用户。
- 每次签发会**追加一行台账**到 `license_ledger.csv`（与私钥同目录，不入库），字段：`iat, sub, bind, seats, nonce, note`。换机重签、泄露溯源都查它。

**示例**：用户提交的设备指纹（机器码）为 `IL5OBPZCJF`，订单号 `AFD20260807-001`，签发命令：

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink\oss
.\.venv\Scripts\python.exe scripts\license\issue.py `
    --sub IL5OBPZCJF --bind fingerprint --seats 3 --note AFD20260807-001
```

执行后终端输出形如 `SLPRO-XXXX-XXXX-…` 的激活码，回发给用户即可；该码仅能在指纹为 `IL5OBPZCJF` 的设备上激活（含 3 台配额）。

参数说明：

| 参数 | 说明 |
|---|---|
| `--key` | 私钥 hex 文件（仓库外） |
| `--sub` | 买家标识。`--bind fingerprint` 时必须为 10 位设备指纹；`--bind order` 时为订单号 |
| `--bind` | `fingerprint`（默认，绑本机）/ `order`（不验硬件，靠订单号社交约束，用于批量预发货） |
| `--seats` | 允许设备数，默认 3 |
| `--note` | 备注（订单号等），仅入台账，不进激活码 |
| `--nonce` | 吊销/溯源编号（默认随机 8 字节 base32）；**要吊销某 key 时用同一个 nonce** |

---

## 4. 管理：台账、换机、吊销

### 4.1 台账（license_ledger.csv）

- 每次签发自动追加；与私钥同目录，**被 `.gitignore` 排除，绝不入库**。
- 用途：换机核对订单、泄露溯源（按 nonce 定位是哪笔订单流出的）、统计签发量。
- 建议与私钥一起离线备份。

### 4.2 换机 / 重装（免费重签，不限次数）

1. 用户提供**原订单号** + **新设备指纹**。
2. 在台账中按订单号（`note`）核对购买记录。
3. 用新指纹重新执行 `issue.py` 签发，回发新激活码。
4. 原激活码对原设备仍永久有效（在 3 台配额内）；超限时可要求用户先在旧设备「设置 → 授权 → 反激活」。

> 用户侧话术与自助指引见 [`08-troubleshooting.md`](./08-troubleshooting.md)「Pro 授权」。

### 4.3 吊销（key 泄露传播时）

1. 从台账找到泄露 key 的 `nonce`。
2. 把它追加到 [`desktop/src-tauri/src/license/revocation.rs`](../../desktop/src-tauri/src/license/revocation.rs) 的 `REVOKED_NONCES`（**只追加、不改动已有条目**）。
3. 发布新版本——该 nonce 的 key 在新版本下校验为 `Revoked`，降级为免费版。

> 原则（红线 E8）：校验逻辑**只能放宽不能收紧**，吊销只针对已泄露的那一个 key，不得误伤正常 key。**不引入联网校验**（维持零遥测承诺），不升级技术对抗。

---

## 5. 密钥轮换（仅限私钥泄露等极端情况，非常规操作）

正常运营中**永不轮换**（见 §2.1 铁律）。仅当私钥确定泄露、必须切断其签发能力时才执行。公钥一经发布**永不删除**（C1）。若必须轮换：

1. 用 `keygen.py` 生成新密钥对。
2. 把**新公钥追加**到 `PUBKEYS_VENDOR_B64` 数组**末尾**，旧公钥保留。
3. 之后的签发起用新私钥。

客户端验签「任一公钥命中即通过」，因此旧 key 永久有效、新 key 用新公钥验。删除旧公钥 = 所有存量 key 一夜失效，**禁止**。

---

## 6. 跨语言一致性自检（改动签发/验签后必跑）

签发端（Python）与验签端（Rust）是两套实现，任何 base32 / canonical JSON / 格式差异都会导致签出的 key 验不过。改动任一侧后运行：

```powershell
.\.venv\Scripts\python.exe scripts\license\roundtrip_check.py
```

- 用 committed 的**测试私钥**（`scripts/license/test_sk.hex`，公开测试值，无价值）按 fixture 重新签发，与 `test_fixture.json` 逐字比对（Ed25519 确定性签名，任何差异都会不一致）。
- Rust 侧 `license::token::tests::python_fixture_license_validates` 用 ed25519-dalek 验签同一份 fixture，构成 Python 签发 → Rust 验签闭环。
- 该检查已纳入公开 CI（`ci.yml` 的 `version-check` job），无需真实私钥。

---

## 7. 常见问题（作者侧）

**签出的激活码用户粘贴提示无效？**
- 先跑 `roundtrip_check.py` 确认签发/验签一致性。
- 确认 `--sub` 用的是用户**当前**设备指纹（10 位），且 `--bind fingerprint`。
- 让用户确认完整复制 `SLPRO-` 整段（程序会自动忽略空白/换行，但缺段不行）。

**想用「订单号绑定」批量预发货？**
- 用 `--bind order --sub <订单号>`。此模式不比对硬件，任何拿到该 key 的人都能激活（靠订单号形成社交约束），适合提前批量生成、随货发出。

**能限制激活码有效期吗？**
- 买断 key 永不写 `exp`。`exp` 字段仅保留给未来的限时授权（如媒体评测 key），校验代码已就绪；如需限时，临时在 issue.py 的 payload 里加 `exp`（Unix 秒）即可，格式版本无需变。

**台账/私钥误删了？**
- 私钥丢失无法再签发新 key（已发出的仍可用）；台账丢失则失去换机核对与溯源依据。两者都务必离线备份。

---

## 关联文档

- 授权校验技术规格：[`../NewFunctions/monetization/01-engineering-plan.md`](../NewFunctions/monetization/01-engineering-plan.md) §4
- 双仓库维护与编译：[`09-open-core-build.md`](./09-open-core-build.md)
- 用户侧激活/换机排查：[`08-troubleshooting.md`](./08-troubleshooting.md)
- 隐私承诺（离线校验依据）：[`../privacy.md`](../privacy.md)
