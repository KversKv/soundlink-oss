import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface OutputDevice {
  id: string;
  name: string;
}

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

interface CaptureSourceInfo {
  id: string;
  name: string;
  available: boolean;
}

type JitterMode = "low" | "balanced" | "stable" | "auto";
type Role = "receiver" | "sender";

const JITTER_MODES: { value: JitterMode; label: string; desc: string }[] = [
  { value: "low", label: "低延迟", desc: "40ms" },
  { value: "balanced", label: "平衡", desc: "80ms" },
  { value: "stable", label: "稳定", desc: "150ms" },
  { value: "auto", label: "自适应", desc: "动态" },
];

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

export default function App() {
  const [role, setRole] = useState<Role>("receiver");

  const [running, setRunning] = useState(false);
  const [pairingCode, setPairingCode] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [devices, setDevices] = useState<OutputDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [jitterMode, setJitterMode] = useState<JitterMode>("balanced");
  const [volume, setVolume] = useState<number>(100);

  const [senderRunning, setSenderRunning] = useState(false);
  const [senderStatus, setSenderStatus] = useState<SenderStatus | null>(null);
  const [receiverAddr, setReceiverAddr] = useState("");
  const [senderPairingCode, setSenderPairingCode] = useState("");
  const [discovered, setDiscovered] = useState<DiscoveredReceiver[]>([]);
  const [captureSources, setCaptureSources] = useState<CaptureSourceInfo[]>([]);
  const [selectedSource, setSelectedSource] = useState("sine");
  const [discovering, setDiscovering] = useState(false);

  const [error, setError] = useState<string>("");

  useEffect(() => {
    invoke<OutputDevice[]>("list_output_devices")
      .then(setDevices)
      .catch((e) => setError(String(e)));
    invoke<CaptureSourceInfo[]>("list_capture_sources")
      .then((srcs) => {
        setCaptureSources(srcs);
        const firstAvail = srcs.find((s) => s.available);
        if (firstAvail) setSelectedSource(firstAvail.id);
      })
      .catch(() => {});
    invoke<string>("get_role")
      .then((r) => setRole(r as Role))
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<string>("get_jitter_mode")
      .then((m) => setJitterMode(m as JitterMode))
      .catch(() => {});
    invoke<number>("get_volume")
      .then((v) => setVolume(Math.round(v * 100)))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      invoke<ReceiverStatus>("get_status")
        .then(setStatus)
        .catch((e) => setError(String(e)));
    }, 500);
    return () => clearInterval(id);
  }, [running]);

  useEffect(() => {
    if (!senderRunning) return;
    const id = setInterval(() => {
      invoke<SenderStatus>("get_sender_status")
        .then(setSenderStatus)
        .catch((e) => setError(String(e)));
    }, 500);
    return () => clearInterval(id);
  }, [senderRunning]);

  async function switchRole(r: Role) {
    setRole(r);
    await invoke("set_role", { role: r }).catch(() => {});
  }

  async function start() {
    setError("");
    try {
      const r = await invoke<StartResult>("start_receiver");
      setPairingCode(r.pairing_code);
      setDeviceId(r.device_id);
      setRunning(true);
    } catch (e) {
      setError(String(e));
    }
  }

  async function stop() {
    setError("");
    try {
      await invoke("stop_receiver");
      setRunning(false);
      setStatus(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function refreshCode() {
    try {
      const c = await invoke<string>("get_pairing_code");
      setPairingCode(c);
    } catch (e) {
      setError(String(e));
    }
  }

  async function pickDevice(idx: number) {
    setSelectedDevice(idx);
    try {
      await invoke("select_output_device", { index: idx });
    } catch (e) {
      setError(String(e));
    }
  }

  async function pickJitterMode(mode: JitterMode) {
    setJitterMode(mode);
    try {
      await invoke("set_jitter_mode", { mode });
    } catch (e) {
      setError(String(e));
    }
  }

  async function changeVolume(v: number) {
    setVolume(v);
    try {
      await invoke("set_volume", { volume: v / 100 });
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
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
    try {
      await invoke("start_sender", {
        receiverAddr,
        pairingCode: senderPairingCode,
        captureSource: selectedSource,
      });
      setSenderRunning(true);
    } catch (e) {
      setError(String(e));
    }
  }

  async function stopSender() {
    setError("");
    try {
      await invoke("stop_sender");
      setSenderRunning(false);
      setSenderStatus(null);
    } catch (e) {
      setError(String(e));
    }
  }

  const lossPct = status ? (status.loss_rate * 100).toFixed(1) : "0.0";
  const bitrateKbps = status ? Math.round(status.bitrate / 1000) : 0;
  const recBitrateKbps = status ? Math.round(status.recommended_bitrate / 1000) : 0;
  const driftPct = status ? ((status.drift_ratio - 1) * 100).toFixed(2) : "0.00";
  const senderBitrateKbps = senderStatus ? Math.round(senderStatus.bitrate / 1000) : 0;
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
        </header>

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
                <small>设备 ID：{deviceId || "RCV-9819"}</small>
              </div>
            </section>

            <section className="panel-card settings-card">
              <h2>输出设备</h2>
              <label className="field-shell">
                <select
                  value={selectedDevice ?? ""}
                  onChange={(e) => pickDevice(Number(e.target.value))}
                >
                  <option value="">默认设备</option>
                  {devices.map((d, i) => (
                    <option key={d.id} value={i}>{d.name}</option>
                  ))}
                </select>
              </label>

              <h2 className="subsection-title">JITTER 模式</h2>
              <div className="jitter-grid">
                {JITTER_MODES.map((m) => (
                  <button
                    key={m.value}
                    className={jitterMode === m.value ? "selected" : ""}
                    onClick={() => pickJitterMode(m.value)}
                    title={m.desc}
                    type="button"
                  >
                    {m.label}
                  </button>
                ))}
              </div>

              <div className="volume-head">
                <h2>音量</h2>
                <strong>{volume}%</strong>
              </div>
              <div className="volume-row">
                <span aria-hidden="true">◖</span>
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={volume}
                  onChange={(e) => changeVolume(Number(e.target.value))}
                  style={{ "--volume": `${volume}%` } as React.CSSProperties}
                  aria-label="音量"
                />
              </div>
            </section>

            <button
              className={`primary-action ${activeReceiver ? "danger" : "success"}`}
              onClick={activeReceiver ? stop : start}
              type="button"
            >
              <span aria-hidden="true">{activeReceiver ? "□" : "▷"}</span>
              {activeReceiver ? "停止接收" : "开始接收"}
            </button>

            {status && (
              <section className="panel-card stats-card">
                <h2>状态</h2>
                <div className="stats-grid">
                  <StatCard label="状态" value={status.state} />
                  <StatCard label="已收包" value={status.packets_recv} />
                  <StatCard label="丢包" value={`${status.packets_lost}（${lossPct}%）`} />
                  <StatCard label="丢弃" value={status.packets_dropped} />
                  <StatCard label="缓冲" value={`${status.buffer_ms} ms（${status.buffer_depth} 帧）`} />
                  <StatCard label="抖动" value={`${status.jitter_ms} ms`} />
                  <StatCard label="估算延迟" value={`${status.est_latency_ms} ms`} />
                  <StatCard label="接收码率" value={`${bitrateKbps} kbps`} />
                  <StatCard label="建议码率" value={`${recBitrateKbps} kbps${recBitrateKbps > 0 && recBitrateKbps !== 128 ? "（自适应）" : ""}`} />
                  <StatCard label="漂移校正" value={`${driftPct}%`} />
                  <StatCard label="连续 PLC" value={`${status.consecutive_plc} 帧`} />
                  <StatCard label="Jitter 模式" value={status.jitter_mode} />
                </div>
              </section>
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
                {discovered.length > 0 ? discovered.map((d) => (
                  <button
                    key={d.device_id}
                    className={receiverAddr === d.control_addr ? "receiver-item selected" : "receiver-item"}
                    onClick={() => setReceiverAddr(d.control_addr)}
                    type="button"
                  >
                    <strong>{d.device_name}</strong>
                    <span>{d.control_addr}</span>
                    <em>{d.pairing_required ? "需配对" : "已信任"}</em>
                  </button>
                )) : (
                  <div className="empty-state">未发现设备，可手动输入地址。</div>
                )}
              </div>
            </section>

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
            </section>

            <button
              className={`primary-action ${activeSender ? "danger" : "send"}`}
              onClick={activeSender ? stopSender : startSender}
              type="button"
            >
              <span aria-hidden="true">{activeSender ? "□" : "▷"}</span>
              {activeSender ? "停止发送" : "开始发送"}
            </button>

            {senderStatus && (
              <section className="panel-card stats-card">
                <h2>发送端状态</h2>
                <div className="stats-grid">
                  <StatCard label="状态" value={senderStatus.state} />
                  <StatCard label="目标" value={senderStatus.receiver_device_name || senderStatus.target_addr} />
                  <StatCard label="已发包" value={senderStatus.packets_sent} />
                  <StatCard label="编码耗时" value={`${senderStatus.encode_ms_avg.toFixed(1)} ms`} />
                  <StatCard label="发送码率" value={`${senderBitrateKbps} kbps`} />
                  <StatCard label="已信任" value={senderStatus.trusted ? "是" : "否"} />
                  {senderStatus.error && <StatCard label="错误" value={senderStatus.error} />}
                </div>
              </section>
            )}
          </div>
        )}

        {error && <div className="error-banner">错误：{error}</div>}

        <footer className="stage-footer">
          阶段 5：桌面发送端（双电脑互传）。运行 <code>cargo run --example phase5_loopback</code> 自测。
        </footer>
      </section>
    </main>
  );
}
