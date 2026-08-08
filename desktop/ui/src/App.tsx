import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import SettingsPanel, { type AppSettings } from "./components/SettingsPanel";
import Onboarding from "./components/Onboarding";
import CloseDialog from "./components/CloseDialog";
import ConfirmWindow from "./features/quickResolution/ConfirmWindow";
import IdentifyOverlay from "./features/quickResolution/IdentifyOverlay";
import { mapError } from "./utils/errorMap";

interface StartResult {
  pairing_code: string;
  audio_port: number;
  device_id: string;
}

interface ReceiverStatus {
  state: string;
  packets_recv: number;
  packets_lost: number;
  packets_dropped: number;
  buffer_depth: number;
  buffer_ms: number;
  est_latency_ms: number;
  jitter_ms: number;
  loss_rate: number;
  bitrate: number;
  jitter_mode: string;
  recommended_bitrate: number;
  drift_ratio: number;
  consecutive_plc: number;
}

interface SenderStatus {
  state: string;
  target_addr: string;
  receiver_device_id: string;
  receiver_device_name: string;
  packets_sent: number;
  encode_ms_avg: number;
  bitrate: number;
  recommended_bitrate: number;
  trusted: boolean;
  error: string;
}

interface DiscoveredReceiver {
  device_id: string;
  device_name: string;
  control_addr: string;
  audio_port: number;
  protocol_version: number;
  pairing_required: boolean;
}

interface TrustedReceiver {
  device_id: string;
  identity_pub_b64: string;
  name: string | null;
  last_seen: number;
  host: string | null;
  control_port: number | null;
  audio_port: number | null;
}

interface CaptureSourceInfo {
  id: string;
  name: string;
  available: boolean;
}

interface LocalAddressInfo {
  ip: string;
  control_port: number;
  audio_port: number;
}

type JitterMode = "low" | "balanced" | "stable" | "auto";
type PairingMode = "random" | "fixed";
type Role = "receiver" | "sender";

interface AudioParams {
  sample_rate: number;
  channels: number;
  frame_duration_ms: number;
  bitrate: number;
  jitter_mode: JitterMode;
}

interface DesktopSettings {
  device_name: string;
  role: Role;
  selected_device: number | null;
  jitter_mode: JitterMode;
  volume: number;
  pairing: {
    mode: PairingMode;
    fixed_code: string;
  };
  audio_params: AudioParams;
  last_receiver_addr: string;
  selected_capture_source: string;
}

const DEFAULT_AUDIO_PARAMS: AudioParams = {
  sample_rate: 48000,
  channels: 2,
  frame_duration_ms: 10,
  bitrate: 128000,
  jitter_mode: "balanced",
};

function formatPairingCode(code: string) {
  return code || "────────";
}

function StatCard({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="stat-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

/// E6：空状态占位卡片。
function EmptyState({ hint }: { hint: string }) {
  return (
    <section className="panel-card stats-card empty-state-card">
      <div className="empty-state-icon" aria-hidden="true" style={{ opacity: 0.4, fontSize: 28 }}>
        ○
      </div>
      <p style={{ margin: 0, color: "#888", fontSize: 13 }}>{hint}</p>
    </section>
  );
}

export default function App() {
  // QR-1：独立小窗视图（15s 确认窗 / 识别叠层）直接短路渲染，不加载主 UI。
  const subView = new URLSearchParams(window.location.search).get("view");
  if (subView === "qr-confirm") {
    return <ConfirmWindow />;
  }
  if (subView === "qr-identify") {
    return <IdentifyOverlay />;
  }

  const [role, setRole] = useState<Role>("receiver");

  const [running, setRunning] = useState(false);
  const [pairingCode, setPairingCode] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [selectedDevice, setSelectedDevice] = useState<number | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [pairingMode, setPairingMode] = useState<PairingMode>("random");
  const [fixedPairingCode, setFixedPairingCode] = useState("");
  const [audioParams, setAudioParamsState] = useState<AudioParams>(DEFAULT_AUDIO_PARAMS);

  const [senderRunning, setSenderRunning] = useState(false);
  const [senderStatus, setSenderStatus] = useState<SenderStatus | null>(null);
  const [receiverAddr, setReceiverAddr] = useState("");
  const [senderPairingCode, setSenderPairingCode] = useState("");
  const [discovered, setDiscovered] = useState<DiscoveredReceiver[]>([]);
  const [captureSources, setCaptureSources] = useState<CaptureSourceInfo[]>([]);
  const [selectedSource, setSelectedSource] = useState("sine");
  const [discovering, setDiscovering] = useState(false);
  const [trustedReceivers, setTrustedReceivers] = useState<TrustedReceiver[]>([]);
  // MON-01 S2：设备记忆配额（UI 标注 used/max）。
  const [trustQuota, setTrustQuota] = useState<{ max: number; senders_used: number; receivers_used: number } | null>(null);

  const [error, setError] = useState<string>("");

  const [view, setView] = useState<"main" | "settings">("main");
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  // D4：配对锁定状态。null 表示未锁定；number>0 表示剩余锁定秒数。
  const [pairingLockRemaining, setPairingLockRemaining] = useState<number | null>(null);
  // 本机局域网 IP 地址列表（配对码卡片显示，便于对端手动连接）。
  const [localAddresses, setLocalAddresses] = useState<LocalAddressInfo[]>([]);
  // E5：长任务进行中标记。空字符串表示无任务；非空时禁用所有动作按钮。
  const [actionPending, setActionPending] = useState<string>("");
  // E3：是否显示首次引导。
  const [showOnboarding, setShowOnboarding] = useState(false);
  // F6：DRM 提示模态是否显示（首次点开始发送时弹）。
  const [drmHintOpen, setDrmHintOpen] = useState(false);
  // F6：DRM 提示确认后回调 pending（确认后再执行 startSender）。
  const [drmPendingStart, setDrmPendingStart] = useState<() => void>(() => () => {});
  // I5：公钥不一致提示模态。后端检测到 MITM 时 emit `pubkey-mismatch` 事件。
  const [pubkeyMismatchOpen, setPubkeyMismatchOpen] = useState(false);
  const [pubkeyMismatchInfo, setPubkeyMismatchInfo] = useState<{
    device_id: string;
    device_name: string;
    saved_pub_b64: string;
    recv_pub_b64: string;
  } | null>(null);

  useEffect(() => {
    invoke<CaptureSourceInfo[]>("list_capture_sources")
      .then((srcs) => {
        setCaptureSources(srcs);
        // 发送模式默认采集源：优先 WASAPI Loopback（系统音频），不可用则回退第一个可用源。
        const preferred = srcs.find((s) => s.id === "wasapi" && s.available);
        const fallback = srcs.find((s) => s.available);
        const chosen = preferred ?? fallback;
        if (chosen) setSelectedSource(chosen.id);
      })
      .catch(() => {});
    loadTrustedReceivers();
    loadTrustQuota();
    invoke<DesktopSettings>("get_desktop_settings")
      .then((settings) => {
        setRole(settings.role);
        setSelectedDevice(settings.selected_device);
        setPairingMode(settings.pairing.mode);
        setFixedPairingCode(settings.pairing.fixed_code);
        setAudioParamsState(settings.audio_params);
        setReceiverAddr(settings.last_receiver_addr);
        // 仅当持久化的采集源在当前可用列表中才覆盖（否则保留 wasapi 优先的默认）。
        if (settings.selected_capture_source) {
          setCaptureSources((srcs) => {
            if (srcs.some((s) => s.id === settings.selected_capture_source && s.available)) {
              setSelectedSource(settings.selected_capture_source);
            }
            return srcs;
          });
        }
      })
      .catch(() => {});
    invoke<string>("get_role")
      .then((r) => setRole(r as Role))
      .catch(() => {});
    // D4：启动时查询配对锁定状态，若已锁定则恢复倒计时显示。
    invoke<{ is_locked: boolean; remaining_secs: number; attempts: number }>(
      "get_pairing_lock_status"
    )
      .then((st) => {
        if (st.is_locked && st.remaining_secs > 0) {
          setPairingLockRemaining(st.remaining_secs);
        }
      })
      .catch(() => {});
    // 加载本机局域网 IP 地址列表（配对码卡片显示用）。
    invoke<LocalAddressInfo[]>("get_local_addresses")
      .then(setLocalAddresses)
      .catch(() => {});
  }, []);

  // 监听 Rust 端 emit 的事件：关闭请求 + 托盘「设置…」点击 + identity 加载失败（D5）+ sender 状态变化（D1）+ 配对锁定（D4）。
  useEffect(() => {
    const unlistenClose = listen("close-requested", () => setCloseDialogOpen(true));
    const unlistenTray = listen<{ kind: string }>("tray-menu-click", (e) => {
      if (e.payload.kind === "Settings") setView("settings");
    });
    const unlistenIdentity = listen<{ message: string }>("identity-load-failed", (e) => {
      setError(e.payload.message);
    });
    // D4：配对超限锁定，后端推送剩余秒数。
    const unlistenPairingLock = listen<{ remaining_secs: number; remaining_attempts: number }>(
      "pairing-locked",
      (e) => {
        setPairingLockRemaining(e.payload.remaining_secs);
        setError(`配对已锁定，请在 ${e.payload.remaining_secs} 秒后重试`);
      }
    );
    // D1：sender 状态变化（DISCONNECTED/RECONNECTING/RECONNECT_NOW）。
    const unlistenSenderState = listen<{ state: string; error: string }>(
      "sender-state-changed",
      (e) => {
        const { state, error } = e.payload;
        if (state === "RECONNECTING") {
          setError(`重连中：${error}`);
        } else if (state === "RECONNECT_NOW") {
          // 后端 backoff 倒计时结束，触发自动重连（用 last_receiver_addr + 空配对码走已信任路径）。
          setError(`正在${error}…`);
          invoke("start_sender", {
            receiverAddr,
            pairingCode: "",
            captureSource: selectedSource,
          })
            .then(() => {
              setSenderRunning(true);
              setError("");
            })
            .catch((err) => {
              setError(mapError(err));
            });
        } else if (state === "DISCONNECTED" || state === "ERROR") {
          setSenderRunning(false);
          setError(error);
        }
      }
    );
    // I5：公钥不一致提示（后端已拒绝连接，仅 UI 告知）。
    const unlistenPubkeyMismatch = listen<{
      device_id: string;
      device_name: string;
      saved_pub_b64: string;
      recv_pub_b64: string;
    }>("pubkey-mismatch", (e) => {
      setPubkeyMismatchInfo(e.payload);
      setPubkeyMismatchOpen(true);
    });
    // I2 + MON-01 S13：全局快捷键。动作集由后端能力驱动（免费仅 show-window）。
    const unlistenShortcut = listen<{ kind: string }>("global-shortcut", (e) => {
      const kind = e.payload.kind;
      if (kind === "toggle-role") {
        const next = role === "receiver" ? "sender" : "receiver";
        invoke("set_role", { role: next })
          .then(() => setRole(next))
          .catch((err) => setError(mapError(err)));
      } else if (kind === "show-window") {
        invoke("show_main_window").catch(() => {});
      } else if (kind === "start-stop-receiver") {
        if (running || status) {
          stop();
        } else {
          start();
        }
      } else if (kind === "start-stop-sender") {
        if (senderRunning || senderStatus) {
          stopSender();
        } else {
          // 无手动目标时按 S9 规则自动选择（last_peer 优先）。
          invoke<string | null>("resolve_auto_send_target")
            .then(async (deviceId) => {
              if (!deviceId) {
                setError("没有可自动连接的信任接收端");
                return;
              }
              setReceiverAddr("");
              setSenderPairingCode("");
              await invoke("connect_trusted_receiver", {
                deviceId,
                captureSource: selectedSource,
              });
              setSenderRunning(true);
              loadTrustedReceivers();
            })
            .catch((err) => setError(mapError(err)));
        }
      } else if (kind === "cycle-output-device") {
        invoke<{ id: string; name: string }[]>("list_output_devices")
          .then(async (devices) => {
            if (devices.length === 0) return;
            // selected_device 为数组索引（与 select_output_device 语义一致）。
            const cur = selectedDevice ?? -1;
            const nextIdx = (cur + 1) % devices.length;
            await invoke("select_output_device", { index: nextIdx });
            setSelectedDevice(nextIdx);
            setError(`输出设备已切换：${devices[nextIdx].name}`);
          })
          .catch(() => {});
      } else if (kind === "toggle-mute") {
        invoke<boolean>("toggle_mute")
          .then((muted) => setError(muted ? "已静音" : "已取消静音"))
          .catch(() => {});
      }
    });
    // MON-01 S14：快捷键注册失败（被系统/其他软件占用）明确提示，不静默忽略。
    const unlistenShortcutFailed = listen<{ accelerators: string[] }>(
      "shortcut-register-failed",
      (e) => {
        setError(`快捷键 ${e.payload.accelerators.join("、")} 注册失败（可能被其他程序占用）`);
      }
    );
    // MON-01 S1：设备记忆满时被替换提示。
    const unlistenEvicted = listen<{ device_id: string; name: string | null; max: number }>(
      "trust-device-evicted",
      (e) => {
        setError(
          `设备记忆已满（${e.payload.max} 台）：已替换最久未用的「${e.payload.name || e.payload.device_id}」`
        );
        loadTrustedReceivers();
        loadTrustQuota();
      }
    );
    // MON-01 R5：激活/反激活后即时刷新设置（自动化开关可用性随授权变化）。
    const unlistenLicense = listen("license-changed", () => {
      invoke<AppSettings>("get_app_settings")
        .then(setAppSettings)
        .catch(() => {});
      loadTrustQuota();
    });
    // MON-01 S15：托盘应用配置档后同步 UI 状态。
    const unlistenProfileApplied = listen<{ name: string; restart_required: boolean }>(
      "profile-applied",
      (e) => {
        invoke<DesktopSettings>("get_desktop_settings")
          .then((settings) => {
            setRole(settings.role);
            setSelectedDevice(settings.selected_device);
            setAudioParamsState(settings.audio_params);
          })
          .catch(() => {});
        setError(
          e.payload.restart_required
            ? `已切换到配置档「${e.payload.name}」（部分参数需重启流后生效）`
            : `已切换到配置档「${e.payload.name}」`
        );
      }
    );
    return () => {
      unlistenClose.then((fn) => fn());
      unlistenTray.then((fn) => fn());
      unlistenIdentity.then((fn) => fn());
      unlistenPairingLock.then((fn) => fn());
      unlistenSenderState.then((fn) => fn());
      unlistenPubkeyMismatch.then((fn) => fn());
      unlistenShortcut.then((fn) => fn());
      unlistenShortcutFailed.then((fn) => fn());
      unlistenEvicted.then((fn) => fn());
      unlistenLicense.then((fn) => fn());
      unlistenProfileApplied.then((fn) => fn());
    };
  }, [receiverAddr, selectedSource, role, running, status, senderRunning, senderStatus, selectedDevice]);

  // D4：配对锁定倒计时。每秒减 1，到 0 时清空锁定状态并清除错误提示。
  useEffect(() => {
    if (pairingLockRemaining === null) return;
    if (pairingLockRemaining <= 0) {
      setPairingLockRemaining(null);
      setError("");
      return;
    }
    const timer = setInterval(() => {
      setPairingLockRemaining((prev) => (prev === null ? null : prev - 1));
    }, 1000);
    return () => clearInterval(timer);
  }, [pairingLockRemaining]);

  // 启动自动化（MON-01 S3）：判定已下沉到 Rust。前端只执行 resolve_startup_plan
  // 返回的计划；免费实现恒返回 null，即使手工篡改 app_config.json 也不会自动启动。
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const s = await invoke<AppSettings>("get_app_settings");
        if (cancelled) return;
        setAppSettings(s);
        // E3：未完成首次引导则显示 Onboarding。
        if (!s.onboarding_completed) {
          setShowOnboarding(true);
          return; // 不进入启动计划，等引导完成。
        }
        interface StartupPlan {
          silent: boolean;
          auto_receive: boolean;
          auto_send_to: string | null;
        }
        const plan = await invoke<StartupPlan | null>("resolve_startup_plan");
        if (cancelled || !plan) return;
        // 仅按计划执行既有命令；失败仅打日志不阻塞（E1）。
        if (plan.auto_receive) {
          try {
            const r = await invoke<StartResult>("start_receiver");
            if (cancelled) return;
            setPairingCode(r.pairing_code);
            setDeviceId(r.device_id);
            setRunning(true);
            invoke<LocalAddressInfo[]>("get_local_addresses")
              .then(setLocalAddresses)
              .catch(() => {});
          } catch (e) {
            console.warn("自动启动接收失败：", String(e));
          }
        }
        if (plan.auto_send_to) {
          try {
            await invoke("connect_trusted_receiver", {
              deviceId: plan.auto_send_to,
              captureSource: selectedSource,
            });
            if (cancelled) return;
            setSenderRunning(true);
          } catch (e) {
            console.warn("自动连接上次设备失败：", String(e));
          }
        }
      } catch (e) {
        console.warn("加载 AppSettings 失败：", String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      invoke<ReceiverStatus>("get_status")
        .then(setStatus)
        .catch((e) => setError(mapError(e)));
    }, 500);
    return () => clearInterval(id);
  }, [running]);

  useEffect(() => {
    if (!senderRunning) return;
    const id = setInterval(() => {
      invoke<SenderStatus>("get_sender_status")
        .then(setSenderStatus)
        .catch((e) => setError(mapError(e)));
    }, 500);
    return () => clearInterval(id);
  }, [senderRunning]);

  async function switchRole(r: Role) {
    setRole(r);
    await invoke("set_role", { role: r }).catch(() => {});
  }

  async function start() {
    setError("");
    setActionPending("start");
    try {
      const r = await invoke<StartResult>("start_receiver");
      setPairingCode(r.pairing_code);
      setDeviceId(r.device_id);
      setRunning(true);
      // 启动接收器后刷新本机 IP（覆盖启动时网络未就绪场景）。
      invoke<LocalAddressInfo[]>("get_local_addresses")
        .then(setLocalAddresses)
        .catch(() => {});
    } catch (e) {
      setError(mapError(e));
    } finally {
      setActionPending("");
    }
  }

  async function stop() {
    setError("");
    setActionPending("stop");
    try {
      await invoke("stop_receiver");
      setRunning(false);
      setStatus(null);
    } catch (e) {
      setError(mapError(e));
    } finally {
      setActionPending("");
    }
  }

  async function refreshCode() {
    try {
      const c = await invoke<string>("get_pairing_code");
      setPairingCode(c);
    } catch (e) {
      setError(mapError(e));
    }
  }

  async function savePairingSettings(nextMode = pairingMode, nextCode = fixedPairingCode) {
    setError("");
    if (nextMode === "fixed" && !/^\d{8}$/.test(nextCode)) {
      setError("长期配对码需要 8 位数字");
      return;
    }
    try {
      const settings = await invoke<{ mode: PairingMode; fixed_code: string }>("set_pairing_settings", {
        mode: nextMode,
        fixedCode: nextMode === "fixed" ? nextCode : null,
      });
      setPairingMode(settings.mode);
      setFixedPairingCode(settings.fixed_code);
      if (running) await refreshCode();
    } catch (e) {
      setError(mapError(e));
    }
  }

  async function discoverReceivers() {
    setError("");
    setDiscovering(true);
    try {
      const list = await invoke<DiscoveredReceiver[]>("discover_receivers", {
        durationSecs: 3,
      });
      setDiscovered(list);
      if (list.length > 0 && !receiverAddr) {
        setReceiverAddr(list[0].control_addr);
      }
    } catch (e) {
      setError(mapError(e));
    } finally {
      setDiscovering(false);
    }
  }

  async function startSender() {
    setError("");
    if (!receiverAddr) {
      setError("请输入或选择 Receiver 地址");
      return;
    }
    // F6：首次开始发送时弹 DRM 提示；确认后再实际启动。
    if (appSettings && !appSettings.sender_drm_hint_seen) {
      setDrmPendingStart(() => () => doStartSender());
      setDrmHintOpen(true);
      return;
    }
    doStartSender();
  }

  async function doStartSender() {
    setError("");
    setActionPending("startSender");
    try {
      await invoke("start_sender", {
        receiverAddr,
        pairingCode: senderPairingCode,
        captureSource: selectedSource,
      });
      setSenderRunning(true);
      loadTrustedReceivers();
    } catch (e) {
      setError(mapError(e));
    } finally {
      setActionPending("");
    }
  }

  async function stopSender() {
    setError("");
    setActionPending("stopSender");
    try {
      await invoke("stop_sender");
      setSenderRunning(false);
      setSenderStatus(null);
      loadTrustedReceivers();
    } catch (e) {
      setError(mapError(e));
    } finally {
      setActionPending("");
    }
  }

  async function loadTrustedReceivers() {
    try {
      const list = await invoke<TrustedReceiver[]>("list_trusted_receivers");
      setTrustedReceivers(list);
    } catch {
      // 忽略加载失败
    }
  }

  // MON-01 S2：加载设备记忆配额（used/max 标注）。
  async function loadTrustQuota() {
    try {
      const q = await invoke<{ max: number; senders_used: number; receivers_used: number }>(
        "get_trust_quota"
      );
      setTrustQuota(q);
    } catch {
      // 忽略加载失败
    }
  }

  async function connectTrustedReceiver(dev: TrustedReceiver) {
    setError("");
    if (!dev.host || !dev.control_port) {
      setError("已信任设备缺少连接信息");
      return;
    }
    const addr = `${dev.host}:${dev.control_port}`;
    setReceiverAddr(addr);
    setSenderPairingCode("");
    setActionPending("connectTrusted");
    try {
      await invoke("connect_trusted_receiver", {
        deviceId: dev.device_id,
        captureSource: selectedSource,
      });
      setSenderRunning(true);
      loadTrustedReceivers();
    } catch (e) {
      setError(mapError(e));
    } finally {
      setActionPending("");
    }
  }

  async function removeTrustedReceiver(deviceId: string) {
    setError("");
    try {
      await invoke("remove_trusted_receiver", { deviceId });
      loadTrustedReceivers();
    } catch (e) {
      setError(mapError(e));
    }
  }

  const bitrateKbps = status ? Math.round(status.bitrate / 1000) : 0;
  const senderBitrateKbps = senderStatus ? Math.round(senderStatus.bitrate / 1000) : 0;
  const adaptiveOn = audioParams.jitter_mode === "auto";
  const activeReceiver = running || Boolean(status);
  const activeSender = senderRunning || Boolean(senderStatus);

  return (
    <main className="shell">
      <section className="app-card" aria-label="SoundLink 桌面端">
        <header className="brand-header">
          <div className="brand-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" role="img">
              <path d="M8.5 9.5a5 5 0 0 0 0 5M6 7a8.5 8.5 0 0 0 0 10M15.5 9.5a5 5 0 0 1 0 5M18 7a8.5 8.5 0 0 1 0 10" />
              <circle cx="12" cy="12" r="1.7" />
            </svg>
          </div>
          <h1>SoundLink</h1>
          <p>局域网音频流转</p>
          {view === "main" ? (
            <button
              className="settings-entry"
              onClick={() => setView("settings")}
              type="button"
              aria-label="设置"
            >
              <svg viewBox="0 0 24 24" role="img" aria-hidden="true">
                <path
                  fill="currentColor"
                  d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm9.4 4l1.7-1.3-1.7-2.9-2 .8a7.6 7.6 0 0 0-1.5-.9l-.3-2.1h-3.4l-.3 2.1c-.5.2-1 .5-1.5.9l-2-.8-1.7 2.9L9.6 12c0 .5 0 1 .1 1.5l-1.7 1.3 1.7 2.9 2-.8c.5.4 1 .7 1.5.9l.3 2.1h3.4l.3-2.1c.5-.2 1-.5 1.5-.9l2 .8 1.7-2.9-1.7-1.3c.1-.5.1-1 .1-1.5z"
                />
              </svg>
              <span>设置</span>
            </button>
          ) : (
            <button
              className="back-button"
              onClick={() => setView("main")}
              type="button"
            >
              ← 返回
            </button>
          )}
        </header>

        {showOnboarding ? (
          <Onboarding
            role={role}
            onRoleChange={(r) => setRole(r)}
            selectedDevice={selectedDevice}
            onSelectDevice={(idx) => setSelectedDevice(idx)}
            selectedCaptureSource={selectedSource}
            onSelectCaptureSource={(id) => setSelectedSource(id)}
            onFinish={async () => {
              // 刷新 appSettings（onboarding 内已设 onboarding_completed=true）。
              const s = await invoke<AppSettings>("get_app_settings");
              setAppSettings(s);
              setShowOnboarding(false);
            }}
          />
        ) : view === "settings" ? (
          <SettingsPanel
            settings={appSettings}
            onChange={async (next) => {
              const saved = await invoke<AppSettings>("set_app_settings", {
                closeAction: next.close_action ?? null,
                autoStart: next.auto_start ?? null,
                autoReceiveOnStart: next.auto_receive_on_start ?? null,
                autoSendOnStart: next.auto_send_on_start ?? null,
                onboardingCompleted: next.onboarding_completed ?? null,
                senderDrmHintSeen: next.sender_drm_hint_seen ?? null,
              });
              setAppSettings(saved);
            }}
          />
        ) : (
          <>
            <nav className="role-tabs" aria-label="模式切换">
              {(["receiver", "sender"] as Role[]).map((r) => (
                <button
                  key={r}
                  className={role === r ? "active" : ""}
                  onClick={() => switchRole(r)}
                  type="button"
                >
                  {r === "receiver" ? "接收模式" : "发送模式"}
                </button>
              ))}
            </nav>

        {role === "receiver" && (
          <div className="mode-panel">
            <section className="panel-card pairing-card">
              <div className="section-title-row">
                <h2>配对码</h2>
                <button className="text-button" onClick={refreshCode} disabled={!running} type="button">
                  <span aria-hidden="true">↻</span> 刷新
                </button>
              </div>
              <div className="pairing-display">
                <span>{formatPairingCode(pairingCode)}</span>
                <small>设备 ID：{deviceId || "—"}</small>
              </div>
              {localAddresses.length > 0 && (
                <div className="local-address-list" aria-label="本机局域网地址">
                  <small className="local-address-title">本机地址（供对端手动连接）</small>
                  <ul>
                    {localAddresses.map((addr) => (
                      <li key={addr.ip}>
                        <code>{addr.ip}</code>
                        <span className="local-address-ports">
                          控制 {addr.control_port} · 音频 {addr.audio_port}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {pairingLockRemaining !== null && pairingLockRemaining > 0 && (
                <div
                  className="pairing-locked-card"
                  role="alert"
                  style={{
                    marginTop: 8,
                    padding: "8px 12px",
                    border: "1px solid #d33",
                    borderRadius: 6,
                    background: "#fff0f0",
                    color: "#a00",
                    fontSize: 13,
                  }}
                >
                  配对已锁定，请在 {pairingLockRemaining} 秒后重试。
                </div>
              )}
              <div className="pairing-settings">
                <label>
                  <span>模式</span>
                  <select
                    value={pairingMode}
                    onChange={(e) => {
                      const mode = e.target.value as PairingMode;
                      setPairingMode(mode);
                      if (mode === "random" || /^\d{8}$/.test(fixedPairingCode)) {
                        savePairingSettings(mode, fixedPairingCode);
                      }
                    }}
                  >
                    <option value="random">随机配对码</option>
                    <option value="fixed">长期配对码</option>
                  </select>
                </label>
                <label>
                  <span>长期码</span>
                  <input
                    value={fixedPairingCode}
                    onChange={(e) => setFixedPairingCode(e.target.value.replace(/\D/g, "").slice(0, 8))}
                    onBlur={() => pairingMode === "fixed" && savePairingSettings("fixed", fixedPairingCode)}
                    placeholder="8 位数字"
                    disabled={pairingMode !== "fixed"}
                  />
                </label>
                <small>长期码会保存在本机配置中，可重复使用且不受 120 秒有效期限制；错误 5 次仍会触发 60 秒锁定。</small>
              </div>
            </section>

            <button
              className={`primary-action ${activeReceiver ? "danger" : "success"}`}
              onClick={activeReceiver ? stop : start}
              type="button"
              disabled={!!actionPending}
            >
              <span aria-hidden="true">{actionPending ? "…" : activeReceiver ? "□" : "▷"}</span>
              {actionPending === "start" || actionPending === "stop"
                ? "处理中…"
                : activeReceiver
                ? "停止接收"
                : "开始接收"}
            </button>

            {status ? (
              <section className="panel-card stats-card">
                <h2>状态</h2>
                <div className="stats-grid">
                  <StatCard label="状态" value={status.state} />
                  <StatCard label="估算延迟" value={`${status.est_latency_ms} ms`} />
                  <StatCard label="接收码率" value={`${bitrateKbps} kbps`} />
                </div>
              </section>
            ) : (
              <EmptyState hint={running ? "等待音频流…" : "点击上方按钮开始接收"} />
            )}
          </div>
        )}

        {role === "sender" && (
          <div className="mode-panel">
            <section className="panel-card settings-card">
              <h2>采集源</h2>
              <label className="field-shell">
                <select
                  value={selectedSource}
                  onChange={(e) => setSelectedSource(e.target.value)}
                  disabled={senderRunning}
                >
                  {captureSources.map((s) => (
                    <option key={s.id} value={s.id} disabled={!s.available}>
                      {s.name}{!s.available ? "（不可用）" : ""}
                    </option>
                  ))}
                </select>
              </label>

              <div className="section-title-row scan-row">
                <h2>发现 Receiver</h2>
                <button className="text-button" onClick={discoverReceivers} disabled={discovering || senderRunning} type="button">
                  <span aria-hidden="true">⌕</span> {discovering ? "扫描中" : "扫描局域网"}
                </button>
              </div>

              <div className="receiver-list">
                {discovered.length > 0 ? discovered.map((d) => {
                  const isTrusted = trustedReceivers.some((t) => t.device_id === d.device_id);
                  return (
                  <button
                    key={d.device_id}
                    className={receiverAddr === d.control_addr ? "receiver-item selected" : "receiver-item"}
                    onClick={() => setReceiverAddr(d.control_addr)}
                    type="button"
                  >
                    <strong>{d.device_name}</strong>
                    <span>{d.control_addr}</span>
                    <em>{isTrusted ? "已信任" : d.pairing_required ? "需配对" : "可连接"}</em>
                  </button>
                  );
                }) : (
                  <div className="empty-state">未发现设备，可手动输入地址。</div>
                )}
              </div>
            </section>

            {trustedReceivers.length > 0 && (
              <section className="panel-card settings-card">
                <h2>
                  已信任设备
                  {trustQuota && (
                    <small style={{ fontWeight: "normal", color: "#888", marginLeft: 8 }}>
                      {trustQuota.receivers_used}/{trustQuota.max}
                    </small>
                  )}
                </h2>
                <div className="receiver-list">
                  {trustedReceivers.map((t) => (
                    <div key={t.device_id} className="receiver-item trusted-item">
                      <button
                        className="trusted-main"
                        onClick={() => connectTrustedReceiver(t)}
                        disabled={senderRunning}
                        type="button"
                      >
                        <strong>{t.name || t.device_id}</strong>
                        <span>{t.host && t.control_port ? `${t.host}:${t.control_port}` : t.device_id}</span>
                        <em>已信任 · 一键直连</em>
                      </button>
                      <button
                        className="text-button danger"
                        onClick={() => removeTrustedReceiver(t.device_id)}
                        disabled={senderRunning}
                        type="button"
                        title="移除信任"
                      >
                        移除
                      </button>
                    </div>
                  ))}
                </div>
              </section>
            )}

            <section className="panel-card settings-card compact-card">
              <h2>Receiver 地址</h2>
              <label className="field-shell">
                <input
                  type="text"
                  value={receiverAddr}
                  onChange={(e) => setReceiverAddr(e.target.value)}
                  placeholder="192.168.1.100:47810"
                  disabled={senderRunning}
                />
              </label>

              <h2 className="subsection-title">配对码</h2>
              <label className="field-shell">
                <input
                  type="text"
                  value={senderPairingCode}
                  onChange={(e) => setSenderPairingCode(e.target.value)}
                  placeholder="8 位配对码（已信任设备可留空）"
                  disabled={senderRunning}
                />
              </label>
              {pairingLockRemaining !== null && pairingLockRemaining > 0 && (
                <div
                  className="pairing-locked-card"
                  role="alert"
                  style={{
                    marginTop: 8,
                    padding: "8px 12px",
                    border: "1px solid #d33",
                    borderRadius: 6,
                    background: "#fff0f0",
                    color: "#a00",
                    fontSize: 13,
                  }}
                >
                  配对已锁定，请在 {pairingLockRemaining} 秒后重试。
                </div>
              )}
            </section>

            <button
              className={`primary-action ${activeSender ? "danger" : "send"}`}
              onClick={activeSender ? stopSender : startSender}
              type="button"
              disabled={!!actionPending}
            >
              <span aria-hidden="true">{actionPending ? "…" : activeSender ? "□" : "▷"}</span>
              {actionPending === "startSender" ||
              actionPending === "stopSender" ||
              actionPending === "connectTrusted"
                ? "处理中…"
                : activeSender
                ? "停止发送"
                : "开始发送"}
            </button>

            {senderStatus ? (
              <section className="panel-card stats-card">
                <h2>发送端状态</h2>
                <div className="stats-grid">
                  <StatCard label="状态" value={senderStatus.state} />
                  <StatCard label="目标" value={senderStatus.receiver_device_name || senderStatus.target_addr} />
                  <StatCard
                    label="发送码率"
                    value={`${senderBitrateKbps} kbps${adaptiveOn ? "（自动）" : ""}`}
                  />
                  {senderStatus.error && <StatCard label="错误" value={senderStatus.error} />}
                </div>
              </section>
            ) : (
              <EmptyState hint={senderRunning ? "正在连接…" : "点击上方按钮开始发送"} />
            )}
          </div>
        )}
          </>
        )}

        {closeDialogOpen && (
          <CloseDialog
            onClose={() => setCloseDialogOpen(false)}
            onMinimize={async (remember) => {
              if (remember) await invoke("set_close_action", { action: "minimize" });
              await invoke("minimize_to_tray");
              setCloseDialogOpen(false);
            }}
            onQuit={async (remember) => {
              if (remember) await invoke("set_close_action", { action: "quit" });
              await invoke("quit_app");
            }}
          />
        )}

        {/* F6：DRM 受保护内容提示模态（首次开始发送时弹）。 */}
        {drmHintOpen && (
          <div
            className="close-dialog-overlay"
            role="dialog"
            aria-modal="true"
            style={{
              position: "fixed",
              inset: 0,
              background: "rgba(0,0,0,0.4)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 1000,
            }}
          >
            <div
              className="panel-card"
              style={{ maxWidth: 380, padding: 20, textAlign: "center" }}
            >
              <h3 style={{ marginTop: 0 }}>DRM 受保护内容提示</h3>
              <p style={{ fontSize: 13, color: "#555", lineHeight: 1.6 }}>
                部分受 DRM 保护的应用音频可能无法采集，这是 Windows 系统限制，非软件问题。
              </p>
              <button
                type="button"
                className="primary-action success"
                onClick={async () => {
                  setDrmHintOpen(false);
                  // 标记已展示，避免后续重复弹窗。
                  const saved = await invoke<AppSettings>("set_app_settings", {
                    closeAction: null,
                    autoStart: null,
                    autoReceiveOnStart: null,
                    autoSendOnStart: null,
                    onboardingCompleted: null,
                    senderDrmHintSeen: true,
                  });
                  setAppSettings(saved);
                  // 执行 pending 启动。
                  drmPendingStart();
                }}
              >
                我已了解
              </button>
            </div>
          </div>
        )}

        {/* I5：公钥不一致提示模态（后端检测到 MITM 已拒绝连接，UI 告知并提供「删除并重配对」入口）。 */}
        {pubkeyMismatchOpen && pubkeyMismatchInfo && (
          <div
            className="close-dialog-overlay"
            role="dialog"
            aria-modal="true"
            style={{
              position: "fixed",
              inset: 0,
              background: "rgba(0,0,0,0.4)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 1000,
            }}
          >
            <div
              className="panel-card"
              style={{ maxWidth: 400, padding: 20, textAlign: "center" }}
            >
              <h3 style={{ marginTop: 0, color: "#d33" }}>检测到对端身份变化</h3>
              <p style={{ fontSize: 13, color: "#555", lineHeight: 1.6 }}>
                设备 <strong>{pubkeyMismatchInfo.device_name || "(未知)"}</strong> 的身份公钥与本地保存的不一致，可能存在中间人攻击。已自动拒绝连接。
              </p>
              <p style={{ fontSize: 11, color: "#888", lineHeight: 1.4, wordBreak: "break-all" }}>
                设备 ID：{pubkeyMismatchInfo.device_id}
                <br />
                本地公钥：{pubkeyMismatchInfo.saved_pub_b64.slice(0, 24)}…
                <br />
                对端公钥：{pubkeyMismatchInfo.recv_pub_b64.slice(0, 24)}…
              </p>
              <div style={{ display: "flex", gap: 8, justifyContent: "center", marginTop: 12 }}>
                <button
                  type="button"
                  className="primary-action"
                  style={{ background: "#d33" }}
                  onClick={async () => {
                    // 删除已信任设备后关闭模态，由用户主动重新发起 start_sender。
                    try {
                      await invoke("remove_trusted_receiver", { deviceId: pubkeyMismatchInfo.device_id });
                      setTrustedReceivers((prev) => prev.filter((r) => r.device_id !== pubkeyMismatchInfo.device_id));
                    } catch (e) {
                      setError(mapError(e));
                    }
                    setPubkeyMismatchOpen(false);
                    setPubkeyMismatchInfo(null);
                  }}
                >
                  删除并重配对
                </button>
                <button
                  type="button"
                  className="text-button"
                  onClick={() => {
                    setPubkeyMismatchOpen(false);
                    setPubkeyMismatchInfo(null);
                  }}
                >
                  取消
                </button>
              </div>
            </div>
          </div>
        )}

        {error && <div className="error-banner">错误：{error}</div>}

        <footer className="stage-footer">
          SoundLink · 局域网音频流转
        </footer>
      </section>
    </main>
  );
}
