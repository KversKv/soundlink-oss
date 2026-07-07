// 错误码枚举。对齐 docs/First/11-implementation-spec.md §4。

/// 错误码。
enum ErrorCode {
  ok(1000, 'OK'),
  internal(1001, '内部错误'),
  pairingFailed(1002, '配对码错误/证明校验失败'),
  versionMismatch(1003, '协议版本不兼容'),
  pairingExpired(1004, '配对码过期'),
  pairingLocked(1005, '尝试次数超限'),
  notTrusted(1006, '未配对设备直接请求流'),
  streamRejected(1007, '音频参数不支持'),
  decryptFailed(1008, 'AEAD 校验失败'),
  timeout(1009, '心跳/握手超时');

  final int code;
  final String message;
  const ErrorCode(this.code, this.message);

  static ErrorCode fromCode(int code) => values.firstWhere(
    (e) => e.code == code,
    orElse: () => ErrorCode.internal,
  );
}
