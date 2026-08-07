"""签发 SoundLink Pro license（MON-01 T2）。

格式：SLPRO-<base32(payload_json)>-<base32(ed25519_sig)>
签名对象：payload 的 canonical JSON 原始字节
（canonical = json.dumps(sort_keys=True, separators=(",", ":"))，与 Rust 测试构造一致；
Rust 验签直接对解码后的原始字节验签，不重新序列化）。

用法（项目根 .venv python）：
    .venv/Scripts/python scripts/license/issue.py \
        --key ..\\soundlink-license\\vendor_sk.hex \
        --sub <设备指纹|订单号> --bind fingerprint|order [--seats 3] [--note 订单号]

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
    parser.add_argument("--key", required=True, help="私钥 hex 文件（仓库外）")
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

    with open(key_path, "r", encoding="ascii") as f:
        sk = bytes.fromhex(f.read().strip())
    if len(sk) != 32:
        print("错误：私钥必须为 32 字节 hex", file=sys.stderr)
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
