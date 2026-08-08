// QR-1 分辨率快速切换 · 类型定义（与 Rust model.rs serde camelCase 一一对应）。

export type ModeState =
  | "draft"
  | "validated"
  | "provisioning"
  | "ready"
  | "active"
  | "stale"
  | "failed";

export type ProvisionPath = "system" | "nvapi" | "edid";

export type ColorFormat = "RGB" | "YCbCr444" | "YCbCr422" | "YCbCr420";

export type ScalingMode = "aspect" | "fullscreen" | "centered" | "noscaling";

export type TimingStandard = "auto" | "cvt-rb2" | "cvt-rb3" | "manual";

export interface ManualTiming {
  hFront: number;
  hSync: number;
  hBack: number;
  vFront: number;
  vSync: number;
  vBack: number;
  hSyncPol: boolean;
  vSyncPol: boolean;
}

export interface MonitorKey {
  instance_path: string;
  edid_hash: string;
}

export type ModeTarget =
  | { kind: "primary" }
  | { kind: "index"; index: number }
  | { kind: "key"; key: MonitorKey };

export interface ModeError {
  code: string;
  message: string;
  at: number;
}

export interface DisplayModeEntry {
  id: string;
  label: string;
  width: number;
  height: number;
  refreshHz: number;
  bitDepth?: 8 | 10 | 12;
  colorFormat?: ColorFormat;
  scaling?: ScalingMode;
  target: ModeTarget;
  timingStandard: TimingStandard;
  manualTiming?: ManualTiming;
  state: ModeState;
  provisionPath?: ProvisionPath;
  lastError?: ModeError;
  pinnedToTray: boolean;
  order: number;
  hotkey?: string | null;
  /** 切换此模式时跳过 15 秒确认窗（高危，display.md §7.4 反向开关）。 */
  skipConfirm?: boolean;
  createdAt: number;
  lastUsedAt?: number;
}

export type DscOverride = "auto" | "force-on" | "force-off";

export interface QuickResolutionSettings {
  schemaVersion: number;
  enabled: boolean;
  showInTray: boolean;
  maxTrayItems: number;
  confirmBeforeApply: boolean;
  autoRevertSeconds: number;
  restoreOnAppExit: boolean;
  dscOverride: DscOverride;
  allowEdidOverride: boolean;
  enableGlobalHotkeys: boolean;
  helperInstalled: boolean;
  modes: DisplayModeEntry[];
}

export interface SystemMode {
  width: number;
  height: number;
  refreshHz: number;
  bitsPerPel: number;
}

export interface DisplayLinkInfo {
  laneCount: number;
  ratePerLaneGbps: number;
  linkLabel: string;
  bpc?: number;
  colorFormat?: string;
  availableGbps: number;
  source: string;
}

export type DscState =
  | { state: "active" }
  | { state: "inactive" }
  | { state: "likelyActive"; confidence: number; basis: string[] }
  | { state: "unknown"; reason: string; debug?: string[] }
  | { state: "forcedByUser"; on: boolean };

export interface DisplayInfo {
  index: number;
  key: MonitorKey;
  gdiName: string;
  friendlyName: string;
  isPrimary: boolean;
  current?: SystemMode;
  link?: DisplayLinkInfo;
  dsc: DscState;
  maxPixelClockKhz?: number;
}

export interface ValidationReport {
  ok: boolean;
  errors: string[];
  inSystemList: boolean;
  pixelClockKhz: number;
  exceedsMonitorLimit?: boolean;
  feasibility?: {
    pixelClockKhz: number;
    requiredUncompressedGbps: number;
    availableGbps: number;
    uncompressedOk: boolean;
    requiredDscGbps?: number;
    dscOk?: boolean;
  };
}

export interface ProvisionReport {
  succeeded: string[];
  failed: string[];
  activation: string;
  backupId: string;
}

export type SwitchResult =
  | { result: "applied" }
  | { result: "revertedByTimeout" }
  | { result: "revertedByUser" };

export interface BackupInfo {
  id: string;
  monitorShort: string;
  createdAt: number;
  path: string;
  size: number;
}

export interface QrAvailability {
  available: boolean;
  platformSupported: boolean;
}

/// QrError 序列化形态（serde tag=code）。
export interface QrErrorPayload {
  code: string;
  detail?: unknown;
}

export function parseQrError(e: unknown): { code: string; message: string } {
  if (typeof e === "object" && e !== null && "code" in e) {
    const p = e as QrErrorPayload;
    return { code: String(p.code), message: qrErrorMessage(p) };
  }
  return { code: "Unknown", message: String(e) };
}

function qrErrorMessage(p: QrErrorPayload): string {
  const d = (p.detail ?? {}) as Record<string, unknown>;
  switch (p.code) {
    case "FeatureLocked":
      return "此功能需要 Pro 版";
    case "UnsupportedPlatform":
      return "当前平台不支持分辨率快速切换（仅 Windows）";
    case "NvApiUnavailable":
      return "未检测到 NVIDIA 驱动接口";
    case "NvapiBlockedByDsc":
      return "驱动已禁用自定义分辨率（DSC 启用），将改用 EDID 注入";
    case "BandwidthExceeded":
      return `超出链路带宽：需 ${Number(d.need).toFixed(1)} Gbps，可用 ${Number(d.have).toFixed(1)} Gbps`;
    case "ExceedsMonitorLimit":
      return `超出显示器像素时钟上限 ${d.limit_khz} kHz`;
    case "HelperNotInstalled":
      return "需要一次管理员授权以启用 EDID 注入";
    case "ElevationDenied":
      return "管理员授权被拒绝";
    case "HelperIpc":
      return `辅助进程通信失败：${typeof p.detail === "string" ? p.detail : ""}`;
    case "EdidNoSlot":
      return "EDID 无可用时序槽位";
    case "ActivationRequiresLogoff":
      return "未能确定 EDID 覆盖的生效方式，需注销或重启";
    case "ProvisionVerifyFailed":
      return `预置验证失败，已自动还原（尝试 ${d.attempted} 个模式）`;
    case "ModeNotReady":
      return "该模式尚未预置";
    case "ModeNotRegistered":
      return "系统模式列表中不存在该模式";
    case "BlockedByFullscreenApp":
      return `检测到全屏独占程序，已阻止操作：${d.process}`;
    case "Win32":
      return `Win32 调用 ${d.api} 失败，code=${d.code}`;
    case "AutoReverted":
      return "超时未确认，已自动回滚";
    case "DisplayNotFound":
      return `未找到目标显示器`;
    case "ModeNotFound":
      return "模式不存在";
    case "BadRequest":
      return `参数非法：${typeof p.detail === "string" ? p.detail : ""}`;
    case "Edid":
      return `EDID 解析/编辑失败：${typeof p.detail === "string" ? p.detail : ""}`;
    case "Io":
      return `IO 失败：${typeof p.detail === "string" ? p.detail : ""}`;
    default:
      return String(p.code);
  }
}
