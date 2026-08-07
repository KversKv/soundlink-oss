import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/// MON-01 R7：授权区块（设置页）。三态：社区构建 / Pro 未激活 / Pro 已激活。
/// 校验完全离线，不联网不上报（E2）；校验失败只降级为免费版，不弹阻塞对话框（E1）。

interface LicenseInfo {
  entitlement: "free" | "pro";
  state: "free" | "active" | "invalid" | "expired" | "revoked" | "device_mismatch";
  detail: string | null;
  sub_masked: string | null;
  fingerprint: string;
  pro_build: boolean;
}

const STATE_TEXT: Record<LicenseInfo["state"], string> = {
  free: "未激活",
  active: "已激活 Pro",
  invalid: "授权码无效",
  expired: "授权码已过期",
  revoked: "授权码已被吊销",
  device_mismatch: "授权码绑定的不是本机",
};

export default function LicensePanel() {
  const [info, setInfo] = useState<LicenseInfo | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string>("");
  const [copied, setCopied] = useState(false);

  const load = () => {
    invoke<LicenseInfo>("get_license_status")
      .then(setInfo)
      .catch(() => {});
  };

  useEffect(() => {
    load();
    // 激活/反激活即时刷新，无需重启（R5）。
    const unlisten = listen<LicenseInfo>("license-changed", (e) => setInfo(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const activate = async () => {
    if (!key.trim()) return;
    setBusy(true);
    setFeedback("");
    try {
      const next = await invoke<LicenseInfo>("activate_license", { key: key.trim() });
      setInfo(next);
      if (next.state === "active") {
        setKey("");
        setFeedback("激活成功，Pro 功能已解锁。");
      } else {
        setFeedback(next.detail ? `${STATE_TEXT[next.state]}：${next.detail}` : STATE_TEXT[next.state]);
      }
    } catch (e) {
      setFeedback(`激活失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const deactivate = async () => {
    setBusy(true);
    setFeedback("");
    try {
      const next = await invoke<LicenseInfo>("deactivate_license");
      setInfo(next);
      setFeedback("已反激活，回到免费版。");
    } catch (e) {
      setFeedback(`反激活失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const copyFingerprint = async () => {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.fingerprint);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setFeedback("复制失败，请手动选择复制");
    }
  };

  const openExternal = async (url: string) => {
    try {
      await invoke("plugin:opener|open_url", { url });
    } catch (e) {
      console.warn("打开链接失败：", e);
    }
  };

  if (!info) {
    return (
      <section className="panel-card settings-card" id="license-section">
        <h2>授权</h2>
        <div className="settings-empty">加载中…</div>
      </section>
    );
  }

  // 社区构建：不含 Pro 逻辑，激活入口无意义（R5）。
  if (!info.pro_build) {
    return (
      <section className="panel-card settings-card" id="license-section">
        <h2>授权</h2>
        <small style={{ display: "block", color: "#60718d", lineHeight: 1.6 }}>
          本构建为社区版（自行编译），不含 Pro 功能。核心音频流转完整可用；
          如需自动化增强，请从官网或 GitHub Release 下载官方版本。
        </small>
      </section>
    );
  }

  return (
    <section className="panel-card settings-card" id="license-section">
      <h2>授权</h2>
      <div className="about-row">
        <span className="about-label">当前状态</span>
        <span className="about-value">
          {info.state === "active"
            ? `Pro 已激活${info.sub_masked ? `（${info.sub_masked}）` : ""}`
            : "免费版"}
        </span>
      </div>
      {info.state !== "active" && info.state !== "free" && (
        <small style={{ display: "block", color: "#a00", marginTop: 4 }}>
          {STATE_TEXT[info.state]}
          {info.detail ? `：${info.detail}` : ""}（已按免费版运行，功能不受影响）
        </small>
      )}

      <div className="about-row" style={{ marginTop: 8 }}>
        <span className="about-label">设备指纹</span>
        <button type="button" className="text-button" onClick={copyFingerprint} title="点击复制">
          {info.fingerprint} {copied ? "✓ 已复制" : "⧉"}
        </button>
      </div>
      <small style={{ display: "block", color: "#60718d", lineHeight: 1.6, marginTop: 2 }}>
        指纹为单向哈希短码，不含隐私信息、不会上传。下单时把它提供给卖家用于签发授权码。
      </small>

      {info.state === "active" ? (
        <div style={{ marginTop: 10 }}>
          <button type="button" className="text-button danger" onClick={deactivate} disabled={busy}>
            反激活（换机前先释放）
          </button>
        </div>
      ) : (
        <>
          <label className="field-shell" style={{ marginTop: 8 }}>
            <span>授权码</span>
            <input
              type="text"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="SLPRO-…"
              disabled={busy}
              spellCheck={false}
            />
          </label>
          <div style={{ display: "flex", gap: 12, alignItems: "center", marginTop: 6 }}>
            <button
              type="button"
              className="text-button"
              onClick={activate}
              disabled={busy || !key.trim()}
            >
              {busy ? "校验中…" : "激活"}
            </button>
            <button
              type="button"
              className="text-button"
              onClick={() => openExternal("https://soundlink.example.com/pro")}
            >
              购买 Pro（￥9.99 买断）
            </button>
          </div>
          <small style={{ display: "block", color: "#60718d", lineHeight: 1.6, marginTop: 6 }}>
            授权码完全离线校验，激活后永久有效（含后续版本）；同一授权最多 3 台设备。
          </small>
        </>
      )}

      {feedback && (
        <small style={{ display: "block", marginTop: 8, color: feedback.includes("成功") ? "#2a7" : "#a00" }}>
          {feedback}
        </small>
      )}
    </section>
  );
}
