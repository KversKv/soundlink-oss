"""签发 SoundLink Pro license（MON-01 T2）。

格式：SLPRO-<base32(payload_json)>-<base32(ed25519_sig)>
签名对象：payload 的 canonical JSON 原始字节
（canonical = json.dumps(sort_keys=True, separators=(",", ":"))，与 Rust 测试构造一致；
Rust 验签直接对解码后的原始字节验签，不重新序列化）。

用法（项目根 .venv python）：
    .venv/Scripts/python scripts/license/issue.py \
        --sub <设备指纹|订单号> --bind fingerprint|order [--seats 3] [--note 订单号]
    （私钥默认取私仓 <工作区根>/pro/license/vendor_sk.hex，无需 --key）

输出 license 文本（stdout）+ 追加本地台账 license_ledger.csv（与私钥同目录，不入库）。
"""

import argparse
import base64
import csv
import json
import os
import sys
import time

from ed25519_pure import publickey, signature, _self_test

SKU = "desktop-pro"
LEDGER_NAME = "license_ledger.csv"

# 私仓中写死的签发私钥（唯一权威来源）。任何环境都读这一份，绝不重新随机生成。
# 见私仓 license/README.md。工作区布局：SoundLink/{oss,pro}；本文件在 oss/scripts/license/，
# 向上三级即工作区根。旧布局（私仓目录名 soundlink-pro）仅作存在性回退。
def _default_key_path() -> str:
    ws_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
    candidates = [
        os.path.join(ws_root, "pro", "license", "vendor_sk.hex"),
        os.path.join(ws_root, "soundlink-pro", "license", "vendor_sk.hex"),
    ]
    for path in candidates:
        if os.path.isfile(path):
            return path
    return candidates[0]


DEFAULT_KEY_PATH = _default_key_path()

# 与 DEFAULT_KEY_PATH 私钥对应的公钥 base64（编译期写死进客户端 PUBKEYS_VENDOR_B64）。
# 签发前自检：由私钥推导的公钥必须等于它，不等即私钥被换/拿错，立即中止，防误签无效码。
EXPECTED_PUBKEY_B64 = "wKpxUUe0XZsacDcV2sAKXU9K7wGCiQxUk369M6PJvqU="


def b32_encode(data: bytes) -> str:
    """RFC 4648 base32 无填充（与 Rust base32_encode 对齐）。"""
    return base64.b32encode(data).decode().rstrip("=")


def canonical_payload(payload: dict) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def issue(sk: bytes, sub: str, bind: str, seats: int, iat: int, nonce: str) -> str:
    pk = publickey(sk)
    payload = {
        "v": 1,
        "sku": SKU,
        "iat": iat,
        "exp": None,  # 买断：永不写过期时间（字段保留给未来限时授权）
        "sub": sub,
        "bind": bind,
        "seats": seats,
        "nonce": nonce,
    }
    canonical = canonical_payload(payload)
    sig = signature(canonical, sk, pk)
    return f"SLPRO-{b32_encode(canonical)}-{b32_encode(sig)}"


def main() -> int:
    _self_test()

    parser = argparse.ArgumentParser(description="签发 SoundLink Pro license")
    parser.add_argument(
        "--key",
        default=DEFAULT_KEY_PATH,
        help=f"私钥 hex 文件（默认私仓固定路径：{DEFAULT_KEY_PATH}）",
    )
    parser.add_argument("--sub", required=True, help="买家标识：设备指纹（10 位）或订单号")
    parser.add_argument("--bind", choices=["fingerprint", "order"], required=True)
    parser.add_argument("--seats", type=int, default=3, help="允许设备数（默认 3）")
    parser.add_argument("--note", default="", help="备注（如淘宝/爱发电订单号，仅入台账）")
    parser.add_argument("--iat", type=int, default=None, help="签发时间（默认当前；测试可固定）")
    parser.add_argument("--nonce", default=None, help="吊销/溯源 nonce（默认随机 8 字节 base32）")
    args = parser.parse_args()

    key_path = os.path.abspath(args.key)
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    if key_path.startswith(repo_root + os.sep) and "test" not in os.path.basename(key_path):
        print("警告：私钥位于仓库目录内，存在误提交风险！", file=sys.stderr)

    if not os.path.isfile(key_path):
        print(f"错误：找不到签发私钥：{key_path}", file=sys.stderr)
        print("  默认路径指向私仓 <工作区根>/pro/license/vendor_sk.hex。", file=sys.stderr)
        print("  请确认已克隆私仓（见私仓 license/README.md），或用 --key 显式指定。", file=sys.stderr)
        return 2

    with open(key_path, "r", encoding="ascii") as f:
        sk = bytes.fromhex(f.read().strip())
    if len(sk) != 32:
        print("错误：私钥必须为 32 字节 hex", file=sys.stderr)
        return 2

    # 公钥指纹自检：防"拿错/重生成私钥导致签出的码在已发布软件上验不过"。
    pk_b64 = base64.b64encode(publickey(sk)).decode()
    if key_path == DEFAULT_KEY_PATH and pk_b64 != EXPECTED_PUBKEY_B64:
        print("错误：私仓私钥与客户端内置公钥不匹配！", file=sys.stderr)
        print(f"  推导公钥: {pk_b64}", file=sys.stderr)
        print(f"  期望公钥: {EXPECTED_PUBKEY_B64}", file=sys.stderr)
        print("  说明私钥已被更换/重新生成。请从私仓恢复正确的 vendor_sk.hex 后再签发。", file=sys.stderr)
        return 2

    sub = args.sub.strip().upper()
    if args.bind == "fingerprint" and len(sub) != 10:
        print("错误：指纹绑定时 --sub 必须为 10 位设备指纹", file=sys.stderr)
        return 2

    nonce = (args.nonce or b32_encode(os.urandom(8))).upper()
    iat = args.iat if args.iat is not None else int(time.time())

    license_text = issue(sk, sub, args.bind, args.seats, iat, nonce)

    # 台账（不入库；与私钥同目录）。
    ledger_path = os.path.join(os.path.dirname(key_path), LEDGER_NAME)
    new_file = not os.path.exists(ledger_path)
    with open(ledger_path, "a", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        if new_file:
            w.writerow(["iat", "sub", "bind", "seats", "nonce", "note"])
        w.writerow([iat, sub, args.bind, args.seats, nonce, args.note])

    print(license_text)
    print(f"\n已追加台账：{ledger_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
