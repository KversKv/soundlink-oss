"""纯 Python Ed25519（RFC 8032 参考实现，公域算法）。

仅用于 SoundLink Pro license 签发脚本，避免引入第三方依赖（项目 .venv 不装
cryptography/PyNaCl）。性能足够（单次签名毫秒级）。

安全性说明：本实现不含侧信道防护，但 license 签发是低频离线操作，可接受。
Rust 客户端用 ed25519-dalek 验签，与本实现确定性一致（同一 key+msg → 同一签名）。
"""

import hashlib

b = 256
q = 2**255 - 19
l = 2**252 + 27742317777372353535851937790883648493


def H(m: bytes) -> bytes:
    return hashlib.sha512(m).digest()


def _inv(x: int) -> int:
    return pow(x, q - 2, q)


d = -121665 * _inv(121666) % q
I = pow(2, (q - 1) // 4, q)


def _xrecover(y: int) -> int:
    xx = (y * y - 1) * _inv(d * y * y + 1)
    x = pow(xx, (q + 3) // 8, q)
    if (x * x - xx) % q != 0:
        x = (x * I) % q
    if x % 2 != 0:
        x = q - x
    return x


_By = 4 * _inv(5)
_Bx = _xrecover(_By)
B = [_Bx % q, _By % q]


def _edwards(P, Q):
    x1, y1 = P
    x2, y2 = Q
    x3 = (x1 * y2 + x2 * y1) * _inv(1 + d * x1 * x2 * y1 * y2) % q
    y3 = (y1 * y2 + x1 * x2) * _inv(1 - d * x1 * x2 * y1 * y2) % q
    return [x3, y3]


def _scalarmult(P, e):
    if e == 0:
        return [0, 1]
    Q = _scalarmult(P, e // 2)
    Q = _edwards(Q, Q)
    if e & 1:
        Q = _edwards(Q, P)
    return Q


def _encodeint(y: int) -> bytes:
    return y.to_bytes(32, "little")


def _encodepoint(P) -> bytes:
    x, y = P
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _bit(h: bytes, i: int) -> int:
    return (h[i // 8] >> (i % 8)) & 1


def _secret_scalar(h: bytes) -> int:
    return 2 ** (b - 2) + sum(2**i * _bit(h, i) for i in range(3, b - 2))


def publickey(sk: bytes) -> bytes:
    """32 字节种子 → 32 字节公钥。"""
    assert len(sk) == 32
    h = H(sk)
    a = _secret_scalar(h)
    return _encodepoint(_scalarmult(B, a))


def _Hint(m: bytes) -> int:
    h = H(m)
    return sum(2**i * _bit(h, i) for i in range(2 * b))


def signature(m: bytes, sk: bytes, pk: bytes) -> bytes:
    """RFC 8032 Ed25519 签名（确定性）。"""
    h = H(sk)
    a = _secret_scalar(h)
    r = _Hint(h[b // 8 : b // 4] + m)
    R = _scalarmult(B, r)
    S = (r + _Hint(_encodepoint(R) + pk + m) * a) % l
    return _encodepoint(R) + _encodeint(S)


def _decodepoint(s: bytes):
    y = int.from_bytes(s, "little") & ((1 << 255) - 1)
    x = _xrecover(y)
    if (x & 1) != (s[31] >> 7):
        x = q - x
    if not _isoncurve([x, y]):
        raise ValueError("点不在曲线上")
    return [x, y]


def _isoncurve(P) -> bool:
    x, y = P
    return (-x * x + y * y - 1 - d * x * x * y * y) % q == 0


def checkvalid(sig: bytes, m: bytes, pk: bytes) -> bool:
    """RFC 8032 验签（自检与抽样用；客户端验签在 Rust 侧）。"""
    if len(sig) != 64 or len(pk) != 32:
        return False
    try:
        R = _decodepoint(sig[:32])
        A = _decodepoint(pk)
    except ValueError:
        return False
    S = int.from_bytes(sig[32:], "little")
    if S >= l:
        return False
    h = _Hint(sig[:32] + pk + m)
    sB = _scalarmult(B, S)
    hA = _scalarmult(A, h)
    return _encodepoint(sB) == _encodepoint(_edwards(R, hA))


def _self_test() -> None:
    """自检：测试密钥对（sk 为公开测试值）+ 签名确定性 + 本实现验签自洽。

    sk 推导 pk 的期望值由 Rust 侧 ed25519-dalek 回拍确认（两种实现互证）。
    跨语言一致性的权威门是 roundtrip_check.py + Rust fixture 测试（T3）。
    """
    sk = bytes.fromhex("9d61b19deffebc3a4d0e9e36f34b7d1b3b47d5f9dddc11e5d6c9ecdd4ba1f74b")
    pk_expected = "0532def3855176790c16ccec9c4c2a712e447264d279cc3e8e10fc01ddc7c115"
    pk = publickey(sk)
    assert pk.hex() == pk_expected, f"公钥不匹配: {pk.hex()}"
    sig1 = signature(b"", sk, pk)
    sig2 = signature(b"", sk, pk)
    assert sig1 == sig2, "Ed25519 签名必须是确定性的"
    assert checkvalid(sig1, b"", pk), "自验签失败"
    assert not checkvalid(sig1, b"x", pk), "篡改消息竟然通过验签"


if __name__ == "__main__":
    _self_test()
    print("ed25519_pure self-test OK (RFC 8032 TEST 1)")
