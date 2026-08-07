"""跨语言一致性检查（MON-01 T3，公开 CI 可跑，无需真实私钥）。

1. 用 test_sk.hex（公开测试私钥）按 fixture payload 重新签发 license；
2. 与 test_fixture.json 中已提交的 license 逐字比对（Ed25519 确定性签名，
   任何实现差异——base32/canonical JSON/格式——都会导致不一致）；
3. Rust 侧 `license::token::tests::python_fixture_license_validates` 用
   ed25519-dalek 验签同一份 license，构成 Python 签发 → Rust 验签的闭环。

用法：.venv/Scripts/python scripts/license/roundtrip_check.py
退出码：0 一致；1 不一致；2 用法错误。
"""

import json
import os
import sys

from ed25519_pure import _self_test
from issue import issue

FIXTURE_PATH = os.path.join(os.path.dirname(__file__), "test_fixture.json")
TEST_SK_PATH = os.path.join(os.path.dirname(__file__), "test_sk.hex")


def main() -> int:
    _self_test()

    with open(FIXTURE_PATH, "r", encoding="utf-8") as f:
        fixture = json.load(f)
    with open(TEST_SK_PATH, "r", encoding="ascii") as f:
        sk = bytes.fromhex(f.read().strip())

    p = fixture["payload"]
    regenerated = issue(
        sk,
        sub=p["sub"],
        bind=p["bind"],
        seats=p["seats"],
        iat=p["iat"],
        nonce=p["nonce"],
    )

    if regenerated != fixture["license"]:
        print("不一致！重新签发的 license 与 fixture 不同：", file=sys.stderr)
        print(f"  重新生成: {regenerated}", file=sys.stderr)
        print(f"  fixture : {fixture['license']}", file=sys.stderr)
        return 1

    print("roundtrip OK：Python 签发结果与 fixture 逐字一致。")
    print("（Rust 侧由 license::token::tests::python_fixture_license_validates 验签闭环）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
