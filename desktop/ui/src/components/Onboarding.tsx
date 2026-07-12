import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "./SettingsPanel";

/// E3：采集源信息（与 App.tsx 中 CaptureSourceInfo 对齐）。
interface CaptureSourceInfo {
  id: string;
  name: string;
  available: boolean;
}

/// E3：输出设备信息（与 App.tsx 中 OutputDevice 对齐）。
interface OutputDeviceInfo {
  index: number;
  name: string;
}

interface Props {
  /// 当前角色（onboarding 内可切换）。
  role: "receiver" | "sender";
  onRoleChange: (r: "receiver" | "sender") => void;
  /// 选中的输出设备索引（receiver 模式）。
  selectedDevice: number | null;
  onSelectDevice: (idx: number) => void;
  /// 选中的采集源 id（sender 模式）。
  selectedCaptureSource: string;
  onSelectCaptureSource: (id: string) => void;
  /// 完成回调：onboarding 内已完成 set_role / select_output_device / set_app_settings。
  onFinish: () => void;
}

const TOTAL_STEPS = 3;

export default function Onboarding({
  role,
  onRoleChange,
  selectedDevice,
  onSelectDevice,
  selectedCaptureSource,
  onSelectCaptureSource,
  onFinish,
}: Props) {
  const [step, setStep] = useState(0);
  const [outputDevices, setOutputDevices] = useState<OutputDeviceInfo[]>([]);
  const [captureSources, setCaptureSources] = useState<CaptureSourceInfo[]>([]);
  const [busy, setBusy] = useState(false);
  // F6：DRM 提示已展示（sender 模式步骤 1 自动标记）。
  const [drmHintSeen, setDrmHintSeen] = useState(false);

  useEffect(() => {
    invoke<OutputDeviceInfo[]>("list_output_devices")
      .then(setOutputDevices)
      .catch(() => {});
    invoke<CaptureSourceInfo[]>("list_capture_sources")
      .then((srcs) => {
        setCaptureSources(srcs);
        const firstAvail = srcs.find((s) => s.available);
        if (firstAvail && !selectedCaptureSource) {
          onSelectCaptureSource(firstAvail.id);
        }
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const next = () => setStep((s) => Math.min(s + 1, TOTAL_STEPS - 1));
  const prev = () => setStep((s) => Math.max(s - 1, 0));

  const finish = async () => {
    setBusy(true);
    try {
      // 持久化角色。
      await invoke("set_role", { role });
      // receiver 模式：若选了输出设备，持久化。
      if (role === "receiver" && selectedDevice !== null) {
        await invoke("select_output_device", { index: selectedDevice });
      }
      // F6：sender 模式步骤 1 已展示 DRM 提示，完成时同步设置。
      await invoke<AppSettings>("set_app_settings", {
        closeAction: null,
        autoStart: null,
        autoReceiveOnStart: null,
        autoSendOnStart: null,
        onboardingCompleted: true,
        senderDrmHintSeen: role === "sender" ? true : drmHintSeen,
      });
      onFinish();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="onboarding mode-panel" style={{ maxWidth: 520, margin: "0 auto" }}>
      <header className="onboarding-header" style={{ textAlign: "center", marginBottom: 16 }}>
        <h1 style={{ fontSize: 20, margin: 0 }}>欢迎使用 SoundLink</h1>
        <p style={{ color: "#888", fontSize: 13, marginTop: 4 }}>
          第 {step + 1} / {TOTAL_STEPS} 步
        </p>
      </header>

      {/* 步骤 0：选角色 */}
      {step === 0 && (
        <section className="panel-card settings-card">
          <h2>选择使用模式</h2>
          <div className="radio-group">
            <label className="radio-row">
              <input
                type="radio"
                name="onb-role"
                checked={role === "receiver"}
                onChange={() => onRoleChange("receiver")}
              />
              <span>接收模式（接收其他设备的音频在本机播放）</span>
            </label>
            <label className="radio-row">
              <input
                type="radio"
                name="onb-role"
                checked={role === "sender"}
                onChange={() => onRoleChange("sender")}
              />
              <span>发送模式（采集本机音频发送到其他设备）</span>
            </label>
          </div>
        </section>
      )}

      {/* 步骤 1：选设备/采集源 + DRM 提示（sender） */}
      {step === 1 && (
        <section className="panel-card settings-card">
          <h2>{role === "receiver" ? "选择输出设备" : "选择采集源"}</h2>
          {role === "receiver" ? (
            <label className="field-shell">
              <span>音频输出设备</span>
              <select
                value={selectedDevice ?? ""}
                onChange={(e) => onSelectDevice(Number(e.target.value))}
              >
                {outputDevices.length === 0 ? (
                  <option value="">（无可用设备）</option>
                ) : (
                  outputDevices.map((d) => (
                    <option key={d.index} value={d.index}>
                      {d.name}
                    </option>
                  ))
                )}
              </select>
            </label>
          ) : (
            <>
              <label className="field-shell">
                <span>音频采集源</span>
                <select
                  value={selectedCaptureSource}
                  onChange={(e) => onSelectCaptureSource(e.target.value)}
                >
                  {captureSources.map((s) => (
                    <option key={s.id} value={s.id} disabled={!s.available}>
                      {s.name}
                      {s.available ? "" : "（不可用）"}
                    </option>
                  ))}
                </select>
              </label>
              {/* F6：DRM 受保护内容提示 */}
              <div
                className="drm-hint"
                style={{
                  marginTop: 12,
                  padding: "8px 12px",
                  border: "1px solid #e8c84a",
                  borderRadius: 6,
                  background: "#fffbe8",
                  color: "#7a5b00",
                  fontSize: 13,
                }}
              >
                注意：部分受 DRM 保护的应用音频可能无法采集，这是 Windows 系统限制，非软件问题。
              </div>
            </>
          )}
        </section>
      )}

      {/* 步骤 2：测试连接说明 + 完成 */}
      {step === 2 && (
        <section className="panel-card settings-card">
          <h2>准备就绪</h2>
          {role === "receiver" ? (
            <p style={{ fontSize: 13, lineHeight: 1.6 }}>
              点击下方「完成」后，进入主界面点击「开始接收」生成配对码，
              在发送端输入该配对码即可建立连接。
            </p>
          ) : (
            <p style={{ fontSize: 13, lineHeight: 1.6 }}>
              点击下方「完成」后，进入主界面输入 Receiver 地址与配对码，
              点击「开始发送」即可。
            </p>
          )}
        </section>
      )}

      {/* 导航按钮 */}
      <div
        className="onboarding-nav"
        style={{ display: "flex", justifyContent: "space-between", marginTop: 16 }}
      >
        <button
          type="button"
          className="text-button"
          onClick={prev}
          disabled={step === 0 || busy}
        >
          ← 上一步
        </button>
        {step < TOTAL_STEPS - 1 ? (
          <button
            type="button"
            className="primary-action success"
            onClick={next}
            disabled={busy}
          >
            下一步 →
          </button>
        ) : (
          <button
            type="button"
            className="primary-action success"
            onClick={finish}
            disabled={busy}
          >
            {busy ? "处理中…" : "完成"}
          </button>
        )}
      </div>
    </div>
  );
}
