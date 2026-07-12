/**
 * 错误映射表：把 Rust 端原始错误转为用户友好的中文提示。
 * P0 修复 NF-01 C1：原 `setError(String(e))` 直接暴露 Rust 错误对象，
 * 例如 `Os { code: 10061, kind: ConnectionRefused, message: "由于计算机拒绝..." }`。
 *
 * 匹配规则：按 errorMap 顺序匹配，第一条命中即返回。
 * 未命中返回 fallback 默认提示。
 */

export interface ErrorMapEntry {
  /** 在原始错误字符串中查找的关键字（大小写不敏感）。 */
  match: string;
  /** 友好提示。 */
  message: string;
}

const ERROR_MAP: ErrorMapEntry[] = [
  // 网络连接类
  { match: "ConnectionRefused", message: "对方未开启接收，或防火墙阻挡" },
  { match: "ConnectionReset", message: "连接被对端重置，请重试" },
  { match: "ConnectionAborted", message: "连接被对端中断，请重试" },
  { match: "PermissionDenied", message: "端口被占用或权限不足" },
  { match: "TimedOut", message: "连接超时，请检查网络或对方是否在线" },
  { match: "AddrInUse", message: "端口已被占用，请关闭其他占用端口的程序" },
  { match: "AddrNotAvailable", message: "网络地址不可用" },
  { match: "NetworkUnreachable", message: "网络不可达，请检查网络连接" },
  { match: "HostUnreachable", message: "无法访问目标主机，请检查地址" },
  { match: "NotFound", message: "未找到目标设备或地址" },
  { match: "BrokenPipe", message: "连接管道断裂，请重试" },
  // 配对安全类
  { match: "PairingLocked", message: "配对尝试次数超限，请稍后再试" },
  { match: "PairingExpired", message: "配对码已过期，请刷新后重试" },
  { match: "PairingFailed", message: "配对失败：配对码错误或证明校验失败" },
  { match: "NotTrusted", message: "未配对，请先完成配对流程" },
  { match: "疑似中间人", message: "检测到疑似中间人攻击，已拒绝连接，请删除已信任设备后重新配对" },
  { match: "不可信", message: "对端不可信，请重新配对" },
  { match: "receiver_proof", message: "对端身份校验失败，请重新配对" },
  { match: "身份公钥", message: "对端身份公钥不匹配，请删除该已信任设备后重新配对" },
  // 音频设备类
  { match: "NoDevice", message: "未找到可用的音频设备" },
  { match: "DeviceUnavailable", message: "音频设备不可用，请检查设备连接" },
  { match: "InvalidDevice", message: "音频设备无效，请重新选择" },
  // 配置文件类
  { match: "创建配置目录失败", message: "配置目录创建失败，可能权限不足" },
  { match: "写入配置失败", message: "配置保存失败，可能权限不足或磁盘已满" },
  // 输入校验类
  { match: "固定配对码需要 8 位数字", message: "固定配对码需要 8 位数字" },
  { match: "请输入或选择 Receiver 地址", message: "请输入或选择 Receiver 地址" },
  { match: "已信任设备缺少连接信息", message: "已信任设备缺少连接信息，请重新配对" },
];

const FALLBACK_MESSAGE = "操作失败，请稍后重试";

/**
 * 把任意错误对象转换为用户友好的中文提示。
 *
 * - 已是字符串：直接走映射
 * - Rust 错误对象：取 message 字段后走映射
 * - 其他：toString 后走映射
 */
export function mapError(e: unknown): string {
  let raw: string;
  if (typeof e === "string") {
    raw = e;
  } else if (e instanceof Error) {
    raw = e.message || String(e);
  } else if (e && typeof e === "object" && "message" in e) {
    raw = String((e as { message: unknown }).message);
  } else {
    raw = String(e);
  }

  const lower = raw.toLowerCase();
  for (const entry of ERROR_MAP) {
    if (lower.includes(entry.match.toLowerCase())) {
      return entry.message;
    }
  }
  // 未命中：返回原始错误（截断过长内容）。
  if (raw.length > 200) {
    return raw.slice(0, 200) + "…";
  }
  return raw || FALLBACK_MESSAGE;
}
