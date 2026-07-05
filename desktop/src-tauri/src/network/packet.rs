//! AudioPacket 二进制编解码（UDP 载荷）。
//!
//! 固定头部 32 字节（大端）+ ChaCha20-Poly1305 密文 + 16B 认证标签。
//! 与 `docs/First/11-implementation-spec.md` §2 字节级对齐。

use crate::constants::{
    AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, CHANNELS, CODEC_OPUS, FRAME_DURATION_MS,
    HEADER_LEN, MAGIC, PROTOCOL_VERSION, SAMPLE_RATE,
};
use chacha20poly1305::{
    aead::{Aead, Payload},
    ChaCha20Poly1305, KeyInit, Nonce,
};
use thiserror::Error;

/// flags 位：流末包。
pub const FLAG_STREAM_END: u8 = 0x01;

#[derive(Debug, Error)]
pub enum PacketError {
    #[error("包过短：{0} 字节，需至少 {1}")]
    TooShort(usize, usize),
    #[error("魔数错误：0x{0:04X}")]
    BadMagic(u16),
    #[error("协议版本不兼容：{0}")]
    BadVersion(u8),
    #[error("头部长度错误：{0}，应为 {1}")]
    BadHeaderLen(u8, u8),
    #[error("payload_len 与实际包长不符：声明 {0}，实际 {1}")]
    PayloadLenMismatch(u16, usize),
    #[error("AEAD 解密/校验失败")]
    DecryptFailed,
    #[error("编码失败：{0}")]
    Encode(String),
}

/// AudioPacket 头部（32 字节，大端）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPacketHeader {
    pub stream_id: u32,
    pub sequence: u32,
    pub timestamp: u64,
    pub codec: u8,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub flags: u8,
    pub sample_rate: u32,
    pub payload_len: u16,
}

impl AudioPacketHeader {
    /// 默认基线头部（48kHz/Stereo/Opus/10ms）。
    pub fn new(stream_id: u32, sequence: u32, timestamp: u64) -> Self {
        Self {
            stream_id,
            sequence,
            timestamp,
            codec: CODEC_OPUS,
            channels: CHANNELS,
            frame_duration_ms: FRAME_DURATION_MS,
            flags: 0,
            sample_rate: SAMPLE_RATE,
            payload_len: 0,
        }
    }

    /// 序列化为 32 字节大端头部。
    pub fn to_bytes(&self) -> [u8; HEADER_LEN as usize] {
        let mut buf = [0u8; HEADER_LEN as usize];
        buf[0..2].copy_from_slice(&MAGIC.to_be_bytes());
        buf[2] = PROTOCOL_VERSION;
        buf[3] = HEADER_LEN;
        buf[4..8].copy_from_slice(&self.stream_id.to_be_bytes());
        buf[8..12].copy_from_slice(&self.sequence.to_be_bytes());
        buf[12..20].copy_from_slice(&self.timestamp.to_be_bytes());
        buf[20] = self.codec;
        buf[21] = self.channels;
        buf[22] = self.frame_duration_ms;
        buf[23] = self.flags;
        buf[24..28].copy_from_slice(&self.sample_rate.to_be_bytes());
        buf[28..30].copy_from_slice(&self.payload_len.to_be_bytes());
        // buf[30..32] reserved = 0
        buf
    }

    /// 从 32 字节解析头部并校验 magic/version/header_len。
    pub fn from_bytes(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < HEADER_LEN as usize {
            return Err(PacketError::TooShort(buf.len(), HEADER_LEN as usize));
        }
        let magic = u16::from_be_bytes([buf[0], buf[1]]);
        if magic != MAGIC {
            return Err(PacketError::BadMagic(magic));
        }
        let version = buf[2];
        if version != PROTOCOL_VERSION {
            return Err(PacketError::BadVersion(version));
        }
        let header_len = buf[3];
        if header_len != HEADER_LEN {
            return Err(PacketError::BadHeaderLen(header_len, HEADER_LEN));
        }
        Ok(Self {
            stream_id: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            sequence: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            timestamp: u64::from_be_bytes(buf[12..20].try_into().unwrap()),
            codec: buf[20],
            channels: buf[21],
            frame_duration_ms: buf[22],
            flags: buf[23],
            sample_rate: u32::from_be_bytes([buf[24], buf[25], buf[26], buf[27]]),
            payload_len: u16::from_be_bytes([buf[28], buf[29]]),
        })
    }
}

/// 构造 AEAD nonce：stream_id(4 BE) ‖ sequence(4 BE) ‖ 0x00000000(4)。
pub fn build_nonce(stream_id: u32, sequence: u32) -> [u8; AEAD_NONCE_LEN] {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    nonce[0..4].copy_from_slice(&stream_id.to_be_bytes());
    nonce[4..8].copy_from_slice(&sequence.to_be_bytes());
    // nonce[8..12] 已为 0
    nonce
}

/// 编码 AudioPacket：header ‖ ciphertext ‖ tag。
///
/// - `key`：会话音频密钥（32B）。
/// - `aad`：关联数据 = 32 字节头部原文。
/// - `plaintext`：Opus 帧。
pub fn encode_packet(
    key: &[u8; AEAD_KEY_LEN],
    header: &mut AudioPacketHeader,
    plaintext: &[u8],
) -> Result<Vec<u8>, PacketError> {
    header.payload_len = plaintext.len() as u16;
    let header_bytes = header.to_bytes();
    let nonce = build_nonce(header.stream_id, header.sequence);
    let cipher = ChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &header_bytes,
            },
        )
        .map_err(|e| PacketError::Encode(e.to_string()))?;
    // chacha20poly1305 crate 返回 ciphertext ‖ tag（尾部 16B）。
    let mut out = Vec::with_capacity(HEADER_LEN as usize + ciphertext.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// 解码 AudioPacket：校验头部 → AEAD 解密 → 返回 Opus 帧明文。
pub fn decode_packet(key: &[u8; AEAD_KEY_LEN], buf: &[u8]) -> Result<DecodedPacket, PacketError> {
    let header_len = HEADER_LEN as usize;
    if buf.len() < header_len {
        return Err(PacketError::TooShort(buf.len(), header_len));
    }
    let header = AudioPacketHeader::from_bytes(&buf[..header_len])?;
    let declared = header.payload_len as usize;
    let expected = buf.len() - header_len;
    if declared + AEAD_TAG_LEN != expected {
        return Err(PacketError::PayloadLenMismatch(
            header.payload_len,
            expected,
        ));
    }
    let nonce = build_nonce(header.stream_id, header.sequence);
    let cipher = ChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &buf[header_len..],
                aad: &buf[..header_len],
            },
        )
        .map_err(|_| PacketError::DecryptFailed)?;
    Ok(DecodedPacket { header, plaintext })
}

#[derive(Debug)]
pub struct DecodedPacket {
    pub header: AudioPacketHeader,
    pub plaintext: Vec<u8>, // Opus 帧
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = AudioPacketHeader::new(7, 42, 480 * 42);
        let bytes = h.to_bytes();
        let h2 = AudioPacketHeader::from_bytes(&bytes).unwrap();
        assert_eq!(h, h2);
        assert_eq!(bytes.len(), HEADER_LEN as usize);
    }

    #[test]
    fn packet_encode_decode_roundtrip() {
        let key = [0x42u8; AEAD_KEY_LEN];
        let mut h = AudioPacketHeader::new(1, 5, 2400);
        h.flags = FLAG_STREAM_END;
        let opus_frame = vec![0xABu8; 80];
        let encoded = encode_packet(&key, &mut h, &opus_frame).unwrap();
        // 32 header + 80 cipher + 16 tag = 128
        assert_eq!(encoded.len(), 32 + 80 + 16);
        let decoded = decode_packet(&key, &encoded).unwrap();
        assert_eq!(decoded.header, h);
        assert_eq!(decoded.plaintext, opus_frame);
    }

    #[test]
    fn decode_wrong_key_fails() {
        let key = [0x42u8; AEAD_KEY_LEN];
        let mut h = AudioPacketHeader::new(1, 5, 2400);
        let encoded = encode_packet(&key, &mut h, b"payload").unwrap();
        let bad_key = [0x99u8; AEAD_KEY_LEN];
        assert!(matches!(
            decode_packet(&bad_key, &encoded).unwrap_err(),
            PacketError::DecryptFailed
        ));
    }

    #[test]
    fn bad_magic_rejected() {
        let mut buf = AudioPacketHeader::new(1, 1, 0).to_bytes();
        buf[0] = 0x00;
        assert!(matches!(
            AudioPacketHeader::from_bytes(&buf).unwrap_err(),
            PacketError::BadMagic(_)
        ));
    }
}
