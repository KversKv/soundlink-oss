import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

type JitterMode = "low" | "balanced" | "stable" | "auto";

const JITTER_MODES: { value: JitterMode; label: string; desc: string }[] = [
  { value: "low", label: "低延迟", desc: "40ms" },
  { value: "balanced", label: "平衡", desc: "80ms" },
  { value: "stable", label: "稳定", desc: "150ms" },
  { value: "auto", label: "自适应", desc: "动态" },
];

export default function App() {
  const [running, setRunning] = useState(false);
  const [pairingCode, setPairingCode] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [devices, setDevices] = useState<OutputDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [jitterMode, setJitterMode] = useState<JitterMode>("balanced");
  const [error, setError] = useState<string>("");

  // 列举设备。
  useEffect(() => {
    invoke<OutputDevice[]>("list_output_devices")
      .then(setDevices)
      .catch((e) => setError(String(e)));
  }, []);

  // 获取当前 jitter 模式。
  useEffect(() => {
    invoke<string>("get_jitter_mode")
      .then((m) => setJitterMode(m as JitterMode))
      .catch(() => {});
  }, []);

  // 状态轮询（运行中时）。
  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      invoke<ReceiverStatus>("get_status")
        .then(setStatus)
        .catch((e) => setError(String(e)));
    }, 500);
    return () => clearInterval(id);
  }, [running]);

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

  const lossPct = status ? (status.loss_rate * 100).toFixed(1) : "0.0";
  const bitrateKbps = status ? Math.round(status.bitrate / 1000) : 0;
  const recBitrateKbps = status ? Math.round(status.recommended_bitrate / 1000) : 0;
  const driftPct = status ? ((status.drift_ratio - 1) * 100).toFixed(2) : "0.00";

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", maxWidth: 560, margin: "40px auto", padding: 24 }}>
      <h1 style={{ marginBottom: 4 }}>SoundLink</h1>
      <p style={{ color: "#666", marginTop: 0 }}>局域网音频流转 · 接收器</p>

      <section style={{ margin: "24px 0" }}>
        <h3>配对码</h3>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <code style={{ fontSize: 32, letterSpacing: 4, background: "#f4f4f5", padding: "8px 16px", borderRadius: 8 }}>
            {pairingCode || "— — — — — — — —"}
          </code>
          <button onClick={refreshCode} disabled={!running}>刷新</button>
        </div>
        {deviceId && <p style={{ color: "#888", fontSize: 13 }}>设备 ID：{deviceId}</p>}
      </section>

      <section style={{ margin: "24px 0" }}>
        <h3>输出设备</h3>
        <select
          value={selectedDevice ?? ""}
          onChange={(e) => pickDevice(Number(e.target.value))}
          style={{ width: "100%", padding: 8, fontSize: 14 }}
        >
          <option value="">默认设备</option>
          {devices.map((d, i) => (
            <option key={d.id} value={i}>{d.name}</option>
          ))}
        </select>
      </section>

      <section style={{ margin: "24px 0" }}>
        <h3>Jitter 模式</h3>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {JITTER_MODES.map((m) => (
            <button
              key={m.value}
              onClick={() => pickJitterMode(m.value)}
              style={{
                padding: "6px 12px",
                fontSize: 13,
                cursor: "pointer",
                border: jitterMode === m.value ? "2px solid #22c55e" : "1px solid #ccc",
                background: jitterMode === m.value ? "#f0fdf4" : "#fff",
                borderRadius: 6,
              }}
              title={m.desc}
            >
              {m.label} <span style={{ color: "#888", fontSize: 11 }}>({m.desc})</span>
            </button>
          ))}
        </div>
      </section>

      <section style={{ margin: "24px 0" }}>
        <button
          onClick={running ? stop : start}
          style={{
            padding: "10px 20px", fontSize: 15, cursor: "pointer",
            background: running ? "#ef4444" : "#22c55e", color: "#fff",
            border: "none", borderRadius: 8,
          }}
        >
          {running ? "停止接收" : "开始接收"}
        </button>
      </section>

      {status && (
        <section style={{ margin: "24px 0", background: "#f9fafb", padding: 16, borderRadius: 8 }}>
          <h3 style={{ marginTop: 0 }}>状态</h3>
          <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "4px 12px", fontSize: 14 }}>
            <dt>状态</dt><dd style={{ margin: 0 }}>{status.state}</dd>
            <dt>已收包</dt><dd style={{ margin: 0 }}>{status.packets_recv}</dd>
            <dt>丢包</dt><dd style={{ margin: 0 }}>{status.packets_lost}（{lossPct}%）</dd>
            <dt>丢弃</dt><dd style={{ margin: 0 }}>{status.packets_dropped}</dd>
            <dt>缓冲</dt><dd style={{ margin: 0 }}>{status.buffer_ms} ms（{status.buffer_depth} 帧）</dd>
            <dt>抖动</dt><dd style={{ margin: 0 }}>{status.jitter_ms} ms</dd>
            <dt>估算延迟</dt><dd style={{ margin: 0 }}>{status.est_latency_ms} ms</dd>
            <dt>接收码率</dt><dd style={{ margin: 0 }}>{bitrateKbps} kbps</dd>
            <dt>建议码率</dt><dd style={{ margin: 0 }}>{recBitrateKbps} kbps{recBitrateKbps > 0 && recBitrateKbps !== 128 ? "（自适应）" : ""}</dd>
            <dt>漂移校正</dt><dd style={{ margin: 0 }}>{driftPct}%</dd>
            <dt>连续 PLC</dt><dd style={{ margin: 0 }}>{status.consecutive_plc} 帧</dd>
            <dt>Jitter 模式</dt><dd style={{ margin: 0 }}>{status.jitter_mode}</dd>
          </dl>
        </section>
      )}

      {error && <p style={{ color: "#ef4444" }}>错误：{error}</p>}

      <p style={{ color: "#aaa", fontSize: 12, marginTop: 32 }}>
        阶段 4：体验优化。运行 <code>cargo run --example phase4_loopback</code> 进行弱网自测。
      </p>
    </div>
  );
}
