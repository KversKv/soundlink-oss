package com.soundlink.pairing

// PairingManager — 占位
//
// 职责：配对码输入后的密钥协商（第一版 X25519 + HMAC，后续 SPAKE2/SRP），
// 生成会话密钥，保存设备信任（Keystore / EncryptedSharedPreferences）。
// 下次连接使用已保存信任自动连接。禁止明文记录配对码/密钥。
// 详见 docs/First/05-pairing-security.md
