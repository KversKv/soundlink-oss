import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/// MON-01 S12：配置档面板（PRO-4 多套配置一键切换）。
/// 免费下显示 2 个示例档（灰色不可点）+ Pro 徽标（属演示说明，不产生任何写入）。

interface Profile {
  id: string;
  name: string;
  output_device: number | null;
  jitter_mode: string;
  volume: number;
  role: string;
  peer_device_id: string | null;
}

interface ProfilesInfo {
  available: boolean;
  max: number;
  active_id: string | null;
  profiles: Profile[];
}

interface ApplyProfileResult {
  profile: Profile;
  restart_required: boolean;
}

const DEMO_PROFILES = ["客厅音箱", "桌面耳机"];

export default function ProfilePanel({ available }: { available: boolean }) {
  const [info, setInfo] = useState<ProfilesInfo | null>(null);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState("");

  const load = () => {
    invoke<ProfilesInfo>("list_profiles")
      .then(setInfo)
      .catch(() => {});
  };

  useEffect(() => {
    if (available) load();
  }, [available]);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setFeedback("");
    try {
      await fn();
    } catch (e) {
      setFeedback(String(e));
    } finally {
      setBusy(false);
    }
  };

  const saveCurrent = () =>
    run(async () => {
      await invoke<Profile>("save_profile", { name: newName.trim() });
      setNewName("");
      load();
    });

  const apply = (id: string) =>
    run(async () => {
      const r = await invoke<ApplyProfileResult>("apply_profile", { id });
      setFeedback(
        r.restart_required
          ? `已切换到「${r.profile.name}」，部分参数需重启流后生效`
          : `已切换到「${r.profile.name}」`
      );
      load();
    });

  const remove = (id: string) =>
    run(async () => {
      await invoke("delete_profile", { id });
      load();
    });

  const rename = (p: Profile) =>
    run(async () => {
      const name = window.prompt("重命名配置档", p.name);
      if (!name || name.trim() === p.name) return;
      await invoke("rename_profile", { id: p.id, name: name.trim() });
      load();
    });

  // 免费：示例档灰色展示 + Pro 徽标（S12）。
  if (!available) {
    return (
      <section className="panel-card settings-card">
        <h2>
          配置档
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
        </h2>
        <div className="receiver-list">
          {DEMO_PROFILES.map((name) => (
            <div key={name} className="receiver-item trusted-item" style={{ opacity: 0.5 }}>
              <button className="trusted-main" disabled type="button">
                <strong>{name}</strong>
                <em>示例配置档 · Pro 可用</em>
              </button>
            </div>
          ))}
        </div>
        <small style={{ display: "block", marginTop: 6, color: "#60718d", lineHeight: 1.6 }}>
          保存多套「设备 + Jitter + 音量 + 参数」组合，一键切换。Pro 功能。
        </small>
      </section>
    );
  }

  return (
    <section className="panel-card settings-card">
      <h2>
        配置档
        {info && (
          <small style={{ fontWeight: "normal", color: "#888", marginLeft: 8 }}>
            {info.profiles.length}/{info.max}
          </small>
        )}
      </h2>
      {info && info.profiles.length > 0 ? (
        <div className="receiver-list">
          {info.profiles.map((p) => (
            <div key={p.id} className="receiver-item trusted-item">
              <button
                className="trusted-main"
                onClick={() => apply(p.id)}
                disabled={busy}
                type="button"
              >
                <strong>
                  {info.active_id === p.id ? "✓ " : ""}
                  {p.name}
                </strong>
                <em>
                  {p.role === "receiver" ? "接收" : "发送"} · {p.jitter_mode} · 音量{" "}
                  {Math.round(p.volume * 100)}%
                </em>
              </button>
              <button
                className="text-button"
                onClick={() => rename(p)}
                disabled={busy}
                type="button"
                title="重命名"
              >
                改名
              </button>
              <button
                className="text-button danger"
                onClick={() => remove(p.id)}
                disabled={busy}
                type="button"
                title="删除"
              >
                删除
              </button>
            </div>
          ))}
        </div>
      ) : (
        <small style={{ display: "block", color: "#888" }}>暂无配置档。</small>
      )}
      <label className="field-shell" style={{ marginTop: 8 }}>
        <span>把当前配置保存为档位</span>
        <input
          type="text"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          placeholder="如：客厅音箱"
          disabled={busy}
        />
      </label>
      <button
        type="button"
        className="text-button"
        onClick={saveCurrent}
        disabled={busy || !newName.trim()}
        style={{ marginTop: 4 }}
      >
        保存当前为配置档
      </button>
      {feedback && (
        <small style={{ display: "block", marginTop: 6, color: feedback.startsWith("已切换") ? "#2a7" : "#a00" }}>
          {feedback}
        </small>
      )}
    </section>
  );
}
