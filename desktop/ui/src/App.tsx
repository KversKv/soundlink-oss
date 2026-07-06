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

export default function App() {
  const [role, setRole] = useState<Role>("receiver");

  // Receiver 状态
  const [running, setRunning] = useState(false);
  const [pairingCode, setPairingCode] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [devices, setDevices] = useState<OutputDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [jitterMode, setJitterMode] = useState<JitterMode>("balanced");

  // Sender 状态
  const [senderRunning, setSenderRunning] = useState(false);
  const [senderStatus, setSenderStatus] = useState<SenderStatus | null>(null);
  const [receiverAddr, setReceiverAddr] = useState("");
  const [senderPairingCode, setSenderPairingCode] = useState("");
  const [discovered, setDiscovered] = useState<DiscoveredReceiver[]>([]);
  const [captureSources, setCaptureSources] = useState<CaptureSourceInfo[]>([]);
  const [selectedSource, setSelectedSource] = useState("sine");
  const [discovering, setDiscovering] = useState(false);

  const [error, setError] = useState<string>("");

  // 列举设备 + 采集源 + 角色。
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
  }, []);

  // Receiver 状态轮询。
  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      invoke<ReceiverStatus>("get_status")
        .then(setStatus)
        .catch((e) => setError(String(e)));
    }, 500);
    return () => clearInterval(id);
  }, [running]);

  // Sender 状态轮询。
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

  // ─── Receiver 操作 ───
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

  // ─── Sender 操作 ───
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

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", maxWidth: 560, margin: "40px auto", padding: 24 }}>
      <h1 style={{ marginBottom: 4 }}>SoundLink</h1>
      <p style={{ color: "#666", marginTop: 0 }}>局域网音频流转</p>

      {/* 角色切换 */}
      <section style={{ margin: "16px 0" }}>
        <div style={{ display: "flex", gap: 8 }}>
          {(["receiver", "sender"] as Role[]).map((r) => (
            <button
              key={r}
              onClick={() => switchRole(r)}
              style={{
                padding: "8px 16px",
                fontSize: 14,
                cursor: "pointer",
                border: role === r ? "2px solid #3b82f6" : "1px solid #ccc",
                background: role === r ? "#eff6ff" : "#fff",
                borderRadius: 6,
                fontWeight: role === r ? 600 : 400,
              }}
            >
              {r === "receiver" ? "接收模式" : "发送模式"}
            </button>
          ))}
        </div>
      </section>

      {role === "receiver" && (
        <>
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
        </>
      )}

      {role === "sender" && (
        <>
          <section style={{ margin: "24px 0" }}>
            <h3>采集源</h3>
            <select
              value={selectedSource}
              onChange={(e) => setSelectedSource(e.target.value)}
              style={{ width: "100%", padding: 8, fontSize: 14 }}
            >
              {captureSources.map((s) => (
                <option key={s.id} value={s.id} disabled={!s.available}>
                  {s.name}{!s.available ? "（不可用）" : ""}
                </option>
              ))}
            </select>
          </section>

          <section style={{ margin: "24px 0" }}>
            <h3>发现 Receiver</h3>
            <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
              <button onClick={discoverReceivers} disabled={discovering || senderRunning}>
                {discovering ? "扫描中..." : "扫描局域网"}
              </button>
            </div>
            {discovered.length > 0 && (
              <ul style={{ margin: 0, padding: 0, listStyle: "none" }}>
                {discovered.map((d) => (
                  <li key={d.device_id} style={{ padding: 8, borderBottom: "1px solid #eee", cursor: "pointer" }}
                    onClick={() => setReceiverAddr(d.control_addr)}>
                    <strong>{d.device_name}</strong>
                    <span style={{ color: "#888", fontSize: 12 }}> {d.control_addr} {d.pairing_required ? "· 需配对" : "· 已信任"}</span>
                  </li>
                ))}
              </ul>
            )}
            {discovered.length === 0 && !discovering && (
              <p style={{ color: "#888", fontSize: 13 }}>未发现设备，可手动输入地址。</p>
            )}
          </section>

          <section style={{ margin: "24px 0" }}>
            <h3>Receiver 地址</h3>
            <input
              type="text"
              value={receiverAddr}
              onChange={(e) => setReceiverAddr(e.target.value)}
              placeholder="192.168.1.100:47810"
              disabled={senderRunning}
              style={{ width: "100%", padding: 8, fontSize: 14, boxSizing: "border-box" }}
            />
          </section>

          <section style={{ margin: "24px 0" }}>
            <h3>配对码</h3>
            <input
              type="text"
              value={senderPairingCode}
              onChange={(e) => setSenderPairingCode(e.target.value)}
              placeholder="8 位配对码（已信任设备可留空）"
              disabled={senderRunning}
              style={{ width: "100%", padding: 8, fontSize: 14, boxSizing: "border-box" }}
            />
          </section>

          <section style={{ margin: "24px 0" }}>
            <button
              onClick={senderRunning ? stopSender : startSender}
              style={{
                padding: "10px 20px", fontSize: 15, cursor: "pointer",
                background: senderRunning ? "#ef4444" : "#3b82f6", color: "#fff",
                border: "none", borderRadius: 8,
              }}
            >
              {senderRunning ? "停止发送" : "开始发送"}
            </button>
          </section>

          {senderStatus && (
            <section style={{ margin: "24px 0", background: "#f9fafb", padding: 16, borderRadius: 8 }}>
              <h3 style={{ marginTop: 0 }}>发送端状态</h3>
              <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "4px 12px", fontSize: 14 }}>
                <dt>状态</dt><dd style={{ margin: 0 }}>{senderStatus.state}</dd>
                <dt>目标</dt><dd style={{ margin: 0 }}>{senderStatus.receiver_device_name || senderStatus.target_addr}</dd>
                <dt>已发包</dt><dd style={{ margin: 0 }}>{senderStatus.packets_sent}</dd>
                <dt>编码耗时</dt><dd style={{ margin: 0 }}>{senderStatus.encode_ms_avg.toFixed(1)} ms</dd>
                <dt>发送码率</dt><dd style={{ margin: 0 }}>{senderBitrateKbps} kbps</dd>
                <dt>已信任</dt><dd style={{ margin: 0 }}>{senderStatus.trusted ? "是" : "否"}</dd>
                {senderStatus.error && <><dt>错误</dt><dd style={{ margin: 0, color: "#ef4444" }}>{senderStatus.error}</dd></>}
              </dl>
            </section>
          )}
        </>
      )}

      {error && <p style={{ color: "#ef4444" }}>错误：{error}</p>}

      <p style={{ color: "#aaa", fontSize: 12, marginTop: 32 }}>
        阶段 5：桌面发送端（双电脑互传）。运行 <code>cargo run --example phase5_loopback</code> 自测。
      </p>
    </div>
  );
}
