import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface OutputDevice {
  id: string;
  name: string;
}

type JitterMode = "low" | "balanced" | "stable" | "auto";

export interface AudioParams {
  sample_rate: number;
  channels: number;
  frame_duration_ms: number;
  bitrate: number;
  jitter_mode: JitterMode;
}

const JITTER_MODES: { value: JitterMode; label: string; desc: string }[] = [
  { value: "low", label: "低延迟", desc: "40ms" },
  { value: "balanced", label: "平衡", desc: "80ms" },
  { value: "stable", label: "稳定", desc: "150ms" },
  { value: "auto", label: "自适应", desc: "动态" },
];

const BITRATE_OPTIONS = [64000, 96000, 128000, 160000, 192000];
// 采样率受 Opus 限制固定 48kHz（44100 不被 libopus 支持）。
const SAMPLE_RATE_OPTIONS = [48000];
const CHANNEL_OPTIONS = [1, 2];
const FRAME_DURATION_OPTIONS = [10, 20];

/// 设置页「音频」分区：输出设备 / Jitter / 音量 / 音频参数。
/// 自主管理状态，直接调用后端命令；不与主界面共享状态。
export default function AudioSettingsPanel() {
  const [devices, setDevices] = useState<OutputDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number | null>(null);
  const [jitterMode, setJitterMode] = useState<JitterMode>("balanced");
  const [volume, setVolume] = useState<number>(100);
  const [audioParams, setAudioParamsState] = useState<AudioParams>({
    sample_rate: 48000,
    channels: 2,
    frame_duration_ms: 10,
    bitrate: 128000,
    jitter_mode: "balanced",
  });
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<OutputDevice[]>("list_output_devices")
      .then(setDevices)
      .catch(() => {});
    invoke<{ selected_device: number | null; jitter_mode: JitterMode; volume: number; audio_params: AudioParams }>(
      "get_desktop_settings"
    )
      .then((s) => {
        setSelectedDevice(s.selected_device);
        setJitterMode(s.jitter_mode);
        setVolume(Math.round(s.volume * 100));
        setAudioParamsState(s.audio_params);
      })
      .catch(() => {});
  }, []);

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
    setAudioParamsState((p) => ({ ...p, jitter_mode: mode }));
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

  async function setAudioParams(params: AudioParams) {
    setAudioParamsState(params);
    setJitterMode(params.jitter_mode);
    try {
      const saved = await invoke<AudioParams>("set_audio_params", { params });
      setAudioParamsState(saved);
      setJitterMode(saved.jitter_mode);
    } catch (e) {
      setError(String(e));
    }
  }

  async function autoDetectAudioParams() {
    setError("");
    try {
      const detected = await invoke<AudioParams>("auto_detect_audio_params");
      setAudioParamsState(detected);
      setJitterMode(detected.jitter_mode);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section className="panel-card settings-card">
      <div className="section-title-row">
        <h2>音频</h2>
        <button className="text-button" onClick={autoDetectAudioParams} type="button">
          <span aria-hidden="true">⌁</span> 自动探测
        </button>
      </div>

      <label className="field-shell">
        <span>输出设备（接收模式）</span>
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

      <h2 className="subsection-title">Jitter 模式</h2>
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

      <h2 className="subsection-title">音频参数</h2>
      <div className="audio-options-grid">
        <label>
          <span>采样率</span>
          <select
            value={audioParams.sample_rate}
            onChange={(e) => setAudioParams({ ...audioParams, sample_rate: Number(e.target.value) })}
          >
            {SAMPLE_RATE_OPTIONS.map((v) => <option key={v} value={v}>{v} Hz</option>)}
          </select>
        </label>
        <label>
          <span>声道</span>
          <select
            value={audioParams.channels}
            onChange={(e) => setAudioParams({ ...audioParams, channels: Number(e.target.value) })}
          >
            {CHANNEL_OPTIONS.map((v) => <option key={v} value={v}>{v === 1 ? "Mono" : "Stereo"}</option>)}
          </select>
        </label>
        <label>
          <span>帧长</span>
          <select
            value={audioParams.frame_duration_ms}
            onChange={(e) => setAudioParams({ ...audioParams, frame_duration_ms: Number(e.target.value) })}
          >
            {FRAME_DURATION_OPTIONS.map((v) => <option key={v} value={v}>{v} ms</option>)}
          </select>
        </label>
        <label>
          <span>码率</span>
          <select
            value={audioParams.bitrate}
            onChange={(e) => setAudioParams({ ...audioParams, bitrate: Number(e.target.value) })}
          >
            {BITRATE_OPTIONS.map((v) => <option key={v} value={v}>{Math.round(v / 1000)} kbps</option>)}
          </select>
        </label>
      </div>
      <small className="settings-note">
        码率、Jitter、音量运行时即时生效；采样率/声道/帧长改动需重新开始流后生效。发送端真正生效的为 Opus 码率，其余固定 48kHz/Stereo/10ms。
      </small>
      {error && <div className="error-banner" style={{ marginTop: 8 }}>错误：{error}</div>}
    </section>
  );
}
