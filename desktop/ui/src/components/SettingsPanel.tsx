import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import AudioSettingsPanel from "./AudioSettingsPanel";
import LicensePanel from "./LicensePanel";
import ProfilePanel from "./ProfilePanel";

export interface AppSettings {
  close_action: "ask" | "minimize" | "quit";
  auto_start: boolean;
  auto_receive_on_start: boolean;
  auto_send_on_start: boolean;
  /// E3：是否已完成首次引导。
  onboarding_completed: boolean;
  /// F6：发送端 DRM 提示是否已展示。
  sender_drm_hint_seen: boolean;
  /// MON-01 S4：自动化（自启 + 自动收发）是否可配置（Pro 能力）。
  automation_available: boolean;
  /// MON-01 S10：配置档是否可用（Pro 能力）。
  profiles_available: boolean;
}

/// E1：关于页元信息（由后端 get_app_version 返回）。
interface AppVersionInfo {
  version: string;
  name: string;
  license: string;
  repository: string;
  build_date: string;
}

/// E4：采集源信息（与 App.tsx 中 CaptureSourceInfo 对齐）。
interface CaptureSourceInfo {
  id: string;
  name: string;
  available: boolean;
}

/// E4：桌面设置（仅取所需字段）。
interface DesktopSettings {
  device_name: string;
  selected_capture_source: string;
}

interface Props {
  settings: AppSettings | null;
  onChange: (next: Partial<AppSettings>) => Promise<void>;
}

const CLOSE_ACTION_OPTIONS: { v: AppSettings["close_action"]; label: string }[] = [
  { v: "ask", label: "每次询问" },
  { v: "minimize", label: "最小化到托盘" },
  { v: "quit", label: "退出程序" },
];

export default function SettingsPanel({ settings, onChange }: Props) {
  const [busy, setBusy] = useState(false);
  // E1：关于页元信息。
  const [versionInfo, setVersionInfo] = useState<AppVersionInfo | null>(null);
  // E4：设备名 + 采集源 + 日志。
  const [deviceName, setDeviceName] = useState("");
  const [deviceNameDraft, setDeviceNameDraft] = useState("");
  const [captureSources, setCaptureSources] = useState<CaptureSourceInfo[]>([]);
  const [defaultCaptureSource, setDefaultCaptureSource] = useState("");
  const [logPath, setLogPath] = useState("");
  const [logPreview, setLogPreview] = useState("");
  const [logPreviewOpen, setLogPreviewOpen] = useState(false);
  // I7：日志面板增强 —— 手动刷新、自动刷新、关键字过滤。
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [logFilter, setLogFilter] = useState("");

  useEffect(() => {
    if (!autoRefresh || !logPreviewOpen) return;
    const id = setInterval(() => {
      invoke<string>("get_log_preview", { maxLines: 200 })
        .then(setLogPreview)
        .catch(() => {});
    }, 5000);
    return () => clearInterval(id);
  }, [autoRefresh, logPreviewOpen]);

  useEffect(() => {
    invoke<AppVersionInfo>("get_app_version")
      .then(setVersionInfo)
      .catch(() => {});
    // E4：加载设备名、采集源、默认采集源。
    invoke<DesktopSettings>("get_desktop_settings")
      .then((s) => {
        setDeviceName(s.device_name);
        setDeviceNameDraft(s.device_name);
        setDefaultCaptureSource(s.selected_capture_source);
      })
      .catch(() => {});
    invoke<CaptureSourceInfo[]>("list_capture_sources")
      .then(setCaptureSources)
      .catch(() => {});
    invoke<string>("get_log_path")
      .then(setLogPath)
      .catch(() => {});
  }, []);

  if (!settings) {
    return <div className="settings-empty">加载中…</div>;
  }

  const toggle = async (key: keyof AppSettings, value: boolean) => {
    setBusy(true);
    try {
      await onChange({ [key]: value } as Partial<AppSettings>);
    } finally {
      setBusy(false);
    }
  };

  const setCloseAction = async (v: AppSettings["close_action"]) => {
    setBusy(true);
    try {
      await onChange({ close_action: v });
    } finally {
      setBusy(false);
    }
  };

  // E1：调用 opener 插件打开外部链接。
  const openExternal = async (url: string) => {
    try {
      await invoke("plugin:opener|open_url", { url });
    } catch (e) {
      console.warn("打开链接失败：", e);
    }
  };

  // E4：保存设备名。
  const saveDeviceName = async () => {
    if (!deviceNameDraft.trim() || deviceNameDraft === deviceName) return;
    setBusy(true);
    try {
      await invoke("set_device_name", { name: deviceNameDraft.trim() });
      setDeviceName(deviceNameDraft.trim());
    } catch (e) {
      console.warn("保存设备名失败：", e);
    } finally {
      setBusy(false);
    }
  };

  // E4：切换默认采集源。
  const changeDefaultCaptureSource = async (id: string) => {
    setBusy(true);
    try {
      await invoke("set_default_capture_source", { source: id });
      setDefaultCaptureSource(id);
    } catch (e) {
      console.warn("保存默认采集源失败：", e);
    } finally {
      setBusy(false);
    }
  };

  // E4：打开日志目录。
  const openLogDir = async () => {
    if (!logPath) return;
    try {
      await invoke("plugin:opener|open_path", { path: logPath });
    } catch (e) {
      console.warn("打开日志目录失败：", e);
    }
  };

  // E4：加载日志预览（点击 details 展开时拉取）。
  const loadLogPreview = async () => {
    try {
      const text = await invoke<string>("get_log_preview", { maxLines: 200 });
      setLogPreview(text);
    } catch (e) {
      setLogPreview(`加载失败：${e}`);
    }
  };

  // MON-01 S6：自动化开关为 Pro 能力。免费下置灰 + Pro 徽标 + 说明。
  const automationLocked = !settings.automation_available;

  return (
    <div className="settings-panel mode-panel">
      <section className="panel-card settings-card">
        <h2>
          启动
          {automationLocked && (
            <span
              className="pro-badge"
              title="Pro 功能"
              style={{
                marginLeft: 8,
                fontSize: 11,
                padding: "1px 6px",
                borderRadius: 4,
                background: "#7c5cff",
                color: "#fff",
                verticalAlign: "middle",
              }}
            >
              Pro
            </span>
          )}
        </h2>
        <label className="toggle-row">
          <span>开机自启动</span>
          <input
            type="checkbox"
            checked={settings.auto_start}
            disabled={busy || automationLocked}
            onChange={(e) => toggle("auto_start", e.target.checked)}
          />
        </label>
        <label className="toggle-row">
          <span>自启动后自动开启接收（仅接收模式）</span>
          <input
            type="checkbox"
            checked={settings.auto_receive_on_start}
            disabled={busy || automationLocked || !settings.auto_start}
            onChange={(e) => toggle("auto_receive_on_start", e.target.checked)}
          />
        </label>
        <label className="toggle-row">
          <span>自启动后自动开启发送（仅发送模式）</span>
          <input
            type="checkbox"
            checked={settings.auto_send_on_start}
            disabled={busy || automationLocked || !settings.auto_start}
            onChange={(e) => toggle("auto_send_on_start", e.target.checked)}
          />
        </label>
        {automationLocked && (
          <small style={{ display: "block", marginTop: 6, color: "#60718d", lineHeight: 1.6 }}>
            开机自启与自动收发为 Pro 功能；免费版每次手动一键即可完成同样的操作。
            <button
              type="button"
              className="text-button"
              style={{ marginLeft: 4 }}
              onClick={() => document.getElementById("license-section")?.scrollIntoView({ behavior: "smooth" })}
            >
              了解 Pro
            </button>
          </small>
        )}
      </section>

      <LicensePanel />

      <ProfilePanel available={settings.profiles_available} />

      <section className="panel-card settings-card">
        <h2>关闭窗口行为</h2>
        <div className="radio-group">
          {CLOSE_ACTION_OPTIONS.map((opt) => (
            <label key={opt.v} className="radio-row">
              <input
                type="radio"
                name="close-action"
                checked={settings.close_action === opt.v}
                disabled={busy}
                onChange={() => setCloseAction(opt.v)}
              />
              <span>{opt.label}</span>
            </label>
          ))}
        </div>
      </section>

      <section className="panel-card settings-card">
        <h2>设备</h2>
        <label className="field-shell">
          <span>设备名（mDNS 广播名）</span>
          <input
            type="text"
            value={deviceNameDraft}
            onChange={(e) => setDeviceNameDraft(e.target.value)}
            onBlur={saveDeviceName}
            disabled={busy}
            placeholder="如：MyDesktop"
          />
        </label>
        <label className="field-shell">
          <span>默认采集源（发送模式）</span>
          <select
            value={defaultCaptureSource}
            onChange={(e) => changeDefaultCaptureSource(e.target.value)}
            disabled={busy || captureSources.length === 0}
          >
            {captureSources.length === 0 ? (
              <option value="">（无可用采集源）</option>
            ) : (
              captureSources.map((s) => (
                <option key={s.id} value={s.id} disabled={!s.available}>
                  {s.name}
                  {s.available ? "" : "（不可用）"}
                </option>
              ))
            )}
          </select>
        </label>
      </section>

      <AudioSettingsPanel />

      <section className="panel-card settings-card">
        <h2>日志</h2>
        <div className="about-row">
          <span className="about-label">日志目录</span>
          <button
            type="button"
            className="text-button"
            onClick={openLogDir}
            disabled={!logPath}
          >
            {logPath || "—"}
          </button>
        </div>
        <details
          onToggle={(e) => {
            const open = (e.currentTarget as HTMLDetailsElement).open;
            setLogPreviewOpen(open);
            if (open && !logPreview) loadLogPreview();
          }}
        >
          <summary style={{ display: "inline-block" }}>查看日志预览（最近 200 行）</summary>
          <button
            type="button"
            className="text-button"
            onClick={loadLogPreview}
            disabled={!logPreviewOpen}
            style={{ marginLeft: 8 }}
          >
            ↻ 刷新
          </button>
          <label style={{ display: "block", marginTop: 8 }}>
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              disabled={!logPreviewOpen}
            />{" "}
            自动刷新（5s）
          </label>
          <input
            type="text"
            value={logFilter}
            onChange={(e) => setLogFilter(e.target.value)}
            placeholder="按关键字过滤（大小写不敏感）"
            style={{ width: "100%", marginTop: 4, padding: "4px 6px" }}
          />
          <pre
            className="log-preview"
            style={{
              maxHeight: 240,
              overflow: "auto",
              background: "#f5f5f5",
              padding: 8,
              borderRadius: 4,
              fontSize: 11,
              marginTop: 8,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {logPreviewOpen
              ? (logFilter
                  ? logPreview
                      .split("\n")
                      .filter((l) => l.toLowerCase().includes(logFilter.toLowerCase()))
                      .join("\n")
                  : logPreview) || "加载中…"
              : ""}
          </pre>
        </details>
      </section>

      <section className="panel-card settings-card">
        <h2>使用帮助</h2>
        <div className="about-row">
          <button
            type="button"
            className="text-button"
            onClick={() => openExternal("https://github.com/KversKv/SoundLink/tree/main/docs/First")}
          >
            查看使用文档
          </button>
          <button
            type="button"
            className="text-button"
            onClick={() => openExternal("https://github.com/KversKv/SoundLink/issues")}
          >
            反馈问题
          </button>
        </div>
        <small style={{ display: "block", marginTop: 8, color: "#60718d", lineHeight: 1.6 }}>
          快速上手：接收模式点「开始接收」→ 手机输入配对码；发送模式选采集源 → 输入 Receiver 地址 → 点「开始发送」。
        </small>
      </section>

      <section className="panel-card settings-card">
        <h2>关于</h2>
        {versionInfo ? (
          <div className="about-info">
            <div className="about-row">
              <span className="about-label">版本</span>
              <span className="about-value">{versionInfo.version}</span>
            </div>
            <div className="about-row">
              <span className="about-label">许可证</span>
              <span className="about-value">{versionInfo.license}</span>
            </div>
            <div className="about-row">
              <span className="about-label">构建日期</span>
              <span className="about-value">{versionInfo.build_date}</span>
            </div>
            <div className="about-row">
              <span className="about-label">源代码仓库</span>
              <button
                type="button"
                className="text-button"
                onClick={() => openExternal(versionInfo.repository)}
              >
                {versionInfo.repository}
              </button>
            </div>
          </div>
        ) : (
          <div className="about-info">加载中…</div>
        )}
      </section>
    </div>
  );
}
