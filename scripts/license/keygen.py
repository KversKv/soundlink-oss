"""生成 SoundLink Pro vendor Ed25519 密钥对（MON-01 T1）。

用法（用项目根 .venv 的 python，禁系统 python）：
    .venv/Scripts/python scripts/license/keygen.py --out <仓库外目录>

- 私钥写入 `<out>/vendor_sk.hex`（**绝不入库**；.gitignore 已排除 scripts/license/*.hex 等）。
- 公钥以 base64 打印，填入 desktop/src-tauri/src/license/token.rs 的 PUBKEYS_VENDOR_B64。

⚠️ 私钥丢失 = 无法再签发新 key（已发出的 key 仍永久有效，E8）。
⚠️ 泄露私钥 = 任何人都能签发有效 key。请离线妥善备份。
"""

import argparse
import os
import sys

from ed25519_pure import publickey, _self_test

import base64


def main() -> int:
    _self_test()  # 先自验实现正确性，再生成真密钥。

    parser = argparse.ArgumentParser(description="生成 vendor Ed25519 密钥对")
    parser.add_argument(
        "--out",
        required=True,
        help="私钥输出目录（必须位于仓库之外，如 ..\\soundlink-license）",
    )
    args = parser.parse_args()

    out_dir = os.path.abspath(args.out)
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    if out_dir.startswith(repo_root + os.sep):
        print("错误：--out 必须在仓库目录之外（物理隔离，防误提交）", file=sys.stderr)
        return 2

    os.makedirs(out_dir, exist_ok=True)
    sk_path = os.path.join(out_dir, "vendor_sk.hex")
    if os.path.exists(sk_path):
        print(f"错误：{sk_path} 已存在。如需重新生成，请先手动备份/删除旧私钥。", file=sys.stderr)
        return 2

    sk = os.urandom(32)
    pk = publickey(sk)
    with open(sk_path, "w", encoding="ascii") as f:
        f.write(sk.hex() + "\n")
    # Windows 上尽力限制权限（POSIX 下为 0o600）。
    try:
        os.chmod(sk_path, 0o600)
    except OSError:
        pass

    print("=" * 64)
    print("vendor 密钥对已生成。")
    print(f"私钥（保密！）：{sk_path}")
    print()
    print("公钥 base64（填入 token.rs 的 PUBKEYS_VENDOR_B64）：")
    print(base64.b64encode(pk).decode())
    print("=" * 64)
    print("⚠️ 私钥丢失 = 无法再签发新 key（已发出的 key 仍永久有效）。")
    print("⚠️ 请立即离线备份私钥，并确认它不会被提交进任何 git 仓库。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
