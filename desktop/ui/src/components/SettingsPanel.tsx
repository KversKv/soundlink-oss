import { useState } from "react";

export interface AppSettings {
  close_action: "ask" | "minimize" | "quit";
  auto_start: boolean;
  auto_receive_on_start: boolean;
  auto_send_on_start: boolean;
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

  return (
    <div className="settings-panel mode-panel">
      <section className="panel-card settings-card">
        <h2>启动</h2>
        <label className="toggle-row">
          <span>开机自启动</span>
          <input
            type="checkbox"
            checked={settings.auto_start}
            disabled={busy}
            onChange={(e) => toggle("auto_start", e.target.checked)}
          />
        </label>
        <label className="toggle-row">
          <span>自启动后自动开启接收（仅接收模式）</span>
          <input
            type="checkbox"
            checked={settings.auto_receive_on_start}
            disabled={busy || !settings.auto_start}
            onChange={(e) => toggle("auto_receive_on_start", e.target.checked)}
          />
        </label>
        <label className="toggle-row">
          <span>自启动后自动开启发送（仅发送模式）</span>
          <input
            type="checkbox"
            checked={settings.auto_send_on_start}
            disabled={busy || !settings.auto_start}
            onChange={(e) => toggle("auto_send_on_start", e.target.checked)}
          />
        </label>
      </section>

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
    </div>
  );
}
