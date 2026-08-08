// 快速分辨率切换设置区（display.md §10.1）：显示器选择 + 识别叠层 + 模式列表
// CRUD/拖拽排序/托盘固定 + 从系统导入 + 设置项 + Pro 门控遮罩。

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { qrApi } from "./api";
import ModeEditorDialog from "./ModeEditorDialog";
import type {
  DisplayInfo,
  DisplayModeEntry,
  ModeTarget,
  QrAvailability,
  QuickResolutionSettings,
} from "./types";
import { parseQrError } from "./types";

/// M3：DSC 状态徽标（display.md §6.2 示例文案）。
function DscBadge({ display }: { display: DisplayInfo }) {
  const [open, setOpen] = useState(false);
  const d = display.dsc;
  const link = display.link;
  let text = "DSC 未知";
  let cls = "qr-dsc-unknown";
  if (d.state === "active") { text = "DSC 已启用"; cls = "qr-dsc-on"; }
  else if (d.state === "likelyActive") { text = "DSC 已启用（推断）"; cls = "qr-dsc-on"; }
  else if (d.state === "inactive") { text = "DSC 未启用"; cls = "qr-dsc-off"; }
  else if (d.state === "forcedByUser") { text = d.on ? "DSC 已启用（手动）" : "DSC 未启用（手动）"; cls = "qr-dsc-on"; }
  const linkText = link
    ? ` · ${link.linkLabel}${link.bpc ? ` · ${link.bpc}bpc ${link.colorFormat ?? ""}` : ""} · 可用 ${link.availableGbps.toFixed(1)} Gbps`
    : "";
  return (
    <div className={`qr-dsc-badge ${cls}`}>
      <span className="qr-dsc-dot" />
      <span>{text}{linkText}</span>
      <button type="button" className="text-button" style={{ padding: "0 4px", fontSize: 11 }} onClick={() => setOpen((o) => !o)}>
        诊断
      </button>
      {open && (
        <div className="qr-dsc-detail">
          <div>状态：{d.state}{d.state === "likelyActive" ? `（置信 ${(d.confidence * 100).toFixed(0)}%）` : ""}</div>
          {"reason" in d && <div>原因：{d.reason}</div>}
          {"debug" in d && (d.debug ?? []).length > 0 && (
            <div style={{ marginTop: 4, borderTop: "1px solid var(--line)", paddingTop: 4 }}>
              诊断：{(d.debug ?? []).map((s, i) => <div key={i}>· {s}</div>)}
            </div>
          )}
          {link && (
            <>
              <div>链路：{link.linkLabel}（{link.laneCount} lanes × {link.ratePerLaneGbps} Gbps，{link.source}）</div>
              <div>可用净带宽：{link.availableGbps.toFixed(2)} Gbps</div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function stateLabel(m: DisplayModeEntry, current?: string): { text: string; cls: string } {
  const star = current === m.id ? " ★" : "";
  switch (m.state) {
    case "ready":
      return { text: `✓ 就绪${star}`, cls: "qr-state-ready" };
    case "active":
      return { text: `✓ 生效${star}`, cls: "qr-state-active" };
    case "stale":
      return { text: "需重预置", cls: "qr-state-stale" };
    case "provisioning":
      return { text: "预置中…", cls: "qr-state-pending" };
    case "failed":
      return { text: "失败", cls: "qr-state-failed" };
    case "validated":
      return { text: "待预置", cls: "qr-state-pending" };
    default:
      return { text: "草稿", cls: "qr-state-pending" };
  }
}

export default function QuickResolutionSection() {
  const [avail, setAvail] = useState<QrAvailability | null>(null);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [settings, setSettings] = useState<QuickResolutionSettings | null>(null);
  const [targetIdx, setTargetIdx] = useState<number>(0); // 0=主屏
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<DisplayModeEntry | null>(null);
  const dragId = useRef<string | null>(null);

  const locked = avail !== null && !avail.available;

  const load = useCallback(async () => {
    try {
      const a = await qrApi.getAvailability();
      setAvail(a);
      if (!a.available || !a.platformSupported) return;
      const [d, s] = await Promise.all([qrApi.getDisplays(), qrApi.getSettings()]);
      setDisplays(d);
      setSettings(s);
    } catch (e) {
      setError(parseQrError(e).message);
    }
  }, []);

  useEffect(() => {
    load();
    const un1 = listen("qr://mode-state-changed", () => load());
    const un2 = listen("qr://display-changed", () => load());
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, [load]);

  const persist = useCallback(
    async (next: QuickResolutionSettings) => {
      setBusy(true);
      try {
        const saved = await qrApi.setSettings(next);
        setSettings(saved);
      } catch (e) {
        setError(parseQrError(e).message);
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const currentTarget = (): ModeTarget => {
    if (targetIdx === 0) return { kind: "primary" };
    const d = displays.find((d) => d.index === targetIdx);
    return d ? { kind: "key", key: d.key } : { kind: "index", index: targetIdx };
  };

  const targetDisplay = displays.find((d) =>
    targetIdx === 0 ? d.isPrimary : d.index === targetIdx,
  );

  const modesOfTarget = (settings?.modes ?? []).filter((m) => {
    const t = m.target;
    if (targetIdx === 0) return t.kind === "primary";
    if (t.kind === "index") return t.index === targetIdx;
    if (t.kind === "key") {
      return targetDisplay ? t.key.instance_path === targetDisplay.key.instance_path : false;
    }
    return false;
  });

  // 当前生效模式 id（active 状态）。
  const activeId = modesOfTarget.find((m) => m.state === "active")?.id;

  const onApply = async (id: string) => {
    setBusy(true);
    setError("");
    try {
      const r = await qrApi.apply(id);
      if (r.result === "revertedByTimeout") {
        setError("超时未确认，已自动回滚");
      }
    } catch (e) {
      setError(parseQrError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const onDelete = async (id: string) => {
    setBusy(true);
    try {
      await qrApi.deleteMode(id);
      await load();
    } catch (e) {
      setError(parseQrError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const onToggleTray = async (m: DisplayModeEntry) => {
    if (!settings) return;
    const next = {
      ...settings,
      modes: settings.modes.map((x) => (x.id === m.id ? { ...x, pinnedToTray: !x.pinnedToTray } : x)),
    };
    await persist(next);
  };

  const onImport = async () => {
    setBusy(true);
    setError("");
    try {
      const created = await qrApi.importSystemModes(currentTarget());
      if (created.length === 0) setError("没有可导入的新模式（已存在）");
      await load();
    } catch (e) {
      setError(parseQrError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const onIdentify = async () => {
    try {
      await qrApi.identifyDisplays();
    } catch (e) {
      setError(parseQrError(e).message);
    }
  };

  // 原生拖拽排序。
  const onDrop = async (overId: string) => {
    const src = dragId.current;
    dragId.current = null;
    if (!src || src === overId || !settings) return;
    const ids = modesOfTarget.map((m) => m.id);
    const from = ids.indexOf(src);
    const to = ids.indexOf(overId);
    if (from < 0 || to < 0) return;
    ids.splice(to, 0, ...ids.splice(from, 1));
    // 其它显示器的模式排在后面，保持 order 不交叉。
    const others = settings.modes.filter((m) => !ids.includes(m.id)).map((m) => m.id);
    setBusy(true);
    try {
      await qrApi.reorderModes([...ids, ...others]);
      await load();
    } catch (e) {
      setError(parseQrError(e).message);
    } finally {
      setBusy(false);
    }
  };

  if (!avail) return null;
  if (!avail.platformSupported) return null; // 非 Windows 不展示

  return (
    <section className="panel-card settings-card qr-section">
      <div className="qr-section-head">
        <h2>
          快速分辨率切换
          <span className="pro-badge" style={{ marginLeft: 6, fontSize: 11, padding: "1px 6px", borderRadius: 4, background: "#7c5cff", color: "#fff", verticalAlign: "middle" }}>Pro</span>
        </h2>
        {!locked && settings && (
          <button
            type="button"
            role="switch"
            aria-checked={settings.enabled}
            className={`qr-switch ${settings.enabled ? "qr-switch-on" : ""}`}
            disabled={busy}
            onClick={() => persist({ ...settings, enabled: !settings.enabled })}
          >
            <span className="qr-switch-knob" />
            <span className="qr-switch-label">{settings.enabled ? "开启" : "关闭"}</span>
          </button>
        )}
      </div>

      {locked ? (
        <div className="qr-locked">
          <p>Pro 功能：把任意分辨率/刷新率预置进系统列表后，托盘一键快切（毫秒级生效）。</p>
          <button
            type="button"
            className="text-button"
            onClick={() => document.getElementById("license-section")?.scrollIntoView({ behavior: "smooth" })}
          >
            升级到 Pro 解锁
          </button>
        </div>
      ) : !settings ? (
        <div className="settings-empty">加载中…</div>
      ) : (
        <div className={`qr-content ${!settings.enabled ? "qr-content-off" : ""}`}>
          {/* 关闭态毛玻璃遮挡层 */}
          {!settings.enabled && (
            <div className="qr-off-mask">
              <span>功能已关闭，点右上角开关开启</span>
            </div>
          )}
          {/* 显示器选择 + 识别 */}
          <div className="qr-display-row">
            <label className="field-shell" style={{ flex: 1 }}>
              <span>显示器</span>
              <select
                value={targetIdx}
                disabled={busy}
                onChange={(e) => setTargetIdx(Number(e.target.value))}
              >
                <option value={0}>主显示器</option>
                {displays.map((d) => (
                  <option key={d.index} value={d.index}>
                    {d.index} · {d.friendlyName}
                    {d.isPrimary ? "（主）" : ""}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" className="text-button" onClick={onIdentify} disabled={busy}>
              识别显示器
            </button>
          </div>
          {targetDisplay?.current && (
            <div className="qr-current-line">
              当前：{targetDisplay.current.width}×{targetDisplay.current.height} @
              {targetDisplay.current.refreshHz}Hz
              {targetDisplay.maxPixelClockKhz
                ? `  |  EDID 像素时钟上限 ${(targetDisplay.maxPixelClockKhz / 1e6).toFixed(2)} GPix/s`
                : ""}
            </div>
          )}
          {/* M3：DSC 状态徽标 */}
          {targetDisplay && <DscBadge display={targetDisplay} />}

          {/* 模式列表 */}
          <div className="qr-mode-table">
            <div className="qr-mode-row qr-mode-head">
              <span className="qr-col-drag" />
              <span className="qr-col-label">名称</span>
              <span className="qr-col-res">分辨率</span>
              <span className="qr-col-hz">刷新</span>
              <span className="qr-col-state">状态</span>
              <span className="qr-col-tray">托盘</span>
              <span className="qr-col-ops">操作</span>
            </div>
            {modesOfTarget.length === 0 && (
              <div className="qr-mode-empty">暂无模式，点击下方「添加分辨率」或「从系统导入」。</div>
            )}
            {modesOfTarget.map((m) => {
              const st = stateLabel(m, activeId);
              return (
                <div
                  key={m.id}
                  className="qr-mode-row"
                  draggable
                  onDragStart={() => (dragId.current = m.id)}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={() => onDrop(m.id)}
                >
                  <span className="qr-col-drag" title="拖拽排序">⠿</span>
                  <span className="qr-col-label" title={m.label}>{m.label}</span>
                  <span className="qr-col-res">{m.width}×{m.height}</span>
                  <span className="qr-col-hz">{m.refreshHz}</span>
                  <span className={`qr-col-state ${st.cls}`}>{st.text}</span>
                  <span className="qr-col-tray">
                    <input
                      type="checkbox"
                      checked={m.pinnedToTray}
                      disabled={busy}
                      onChange={() => onToggleTray(m)}
                      title="在托盘菜单中显示"
                    />
                  </span>
                  <span className="qr-col-ops">
                    <button
                      type="button"
                      className="text-button"
                      disabled={busy || !m.state || !(m.state === "ready" || m.state === "active")}
                      onClick={() => onApply(m.id)}
                      title={m.state === "ready" || m.state === "active" ? "立即切换" : "未预置，不能快切"}
                    >
                      ▶
                    </button>
                    <button
                      type="button"
                      className="text-button"
                      disabled={busy}
                      onClick={() => {
                        setEditing(m);
                        setEditorOpen(true);
                      }}
                    >
                      ✎
                    </button>
                    <button
                      type="button"
                      className="text-button"
                      disabled={busy}
                      onClick={() => onDelete(m.id)}
                    >
                      🗑
                    </button>
                  </span>
                </div>
              );
            })}
          </div>

          {error && <div className="qr-feas-line qr-feas-bad">{error}</div>}

          {/* 待预置提示 + 批量预置（M7） */}
          {(() => {
            const pending = modesOfTarget.filter((m) => ["draft", "validated", "failed"].includes(m.state));
            if (pending.length === 0) return null;
            return (
              <div className="qr-feas-line qr-feas-warn" style={{ margin: "4px 0" }}>
                ⚠ 有 {pending.length} 个模式待预置，需重启显示驱动（约 3 秒黑屏）
                <button
                  type="button"
                  className="text-button"
                  style={{ marginLeft: 6 }}
                  disabled={busy}
                  onClick={async () => {
                    setBusy(true);
                    setError("");
                    try {
                      const r = await qrApi.provision(pending.map((m) => m.id));
                      if (r.failed.length > 0) {
                        setError(`预置部分失败：成功 ${r.succeeded.length}，失败 ${r.failed.length}`);
                      }
                      await load();
                    } catch (e) {
                      setError(parseQrError(e).message);
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  立即预置
                </button>
              </div>
            );
          })()}

          <div className="qr-actions-row">
            <button
              type="button"
              className="text-button"
              disabled={busy || !settings.enabled}
              onClick={() => {
                setEditing(null);
                setEditorOpen(true);
              }}
            >
              ＋ 添加分辨率
            </button>
            <button type="button" className="text-button" disabled={busy || !settings.enabled} onClick={onImport}>
              从系统导入
            </button>
            {/* M4：helper 安装（唯一 UAC 入口） */}
            {!settings.helperInstalled && (
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    await invoke("qr_install_helper");
                    setSettings({ ...settings, helperInstalled: true });
                  } catch (e) {
                    setError(parseQrError(e).message);
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                安装辅助组件（一次 UAC）
              </button>
            )}
          </div>

          {/* 设置项 */}
          <div className="qr-settings-block">
            <label className="toggle-row">
              <span>在托盘右键菜单中显示（最多</span>
              <select
                value={settings.maxTrayItems}
                disabled={busy}
                onChange={(e) => persist({ ...settings, maxTrayItems: Number(e.target.value) })}
                style={{ width: 56, margin: "0 4px" }}
              >
                {[4, 6, 8, 12, 16].map((n) => (
                  <option key={n} value={n}>{n}</option>
                ))}
              </select>
              <span>项）</span>
              <input
                type="checkbox"
                checked={settings.showInTray}
                disabled={busy}
                onChange={(e) => persist({ ...settings, showInTray: e.target.checked })}
              />
            </label>
            <label className="toggle-row">
              <span>切换后</span>
              <select
                value={settings.autoRevertSeconds}
                disabled={busy || !settings.confirmBeforeApply}
                onChange={(e) => persist({ ...settings, autoRevertSeconds: Number(e.target.value) })}
                style={{ width: 56, margin: "0 4px" }}
              >
                {[10, 15, 20, 30].map((n) => (
                  <option key={n} value={n}>{n}</option>
                ))}
              </select>
              <span>秒未确认自动回滚</span>
              <input
                type="checkbox"
                checked={settings.confirmBeforeApply}
                disabled={busy}
                onChange={(e) => persist({ ...settings, confirmBeforeApply: e.target.checked })}
              />
            </label>
            <label className="toggle-row">
              <span>退出软件时恢复原始分辨率</span>
              <input
                type="checkbox"
                checked={settings.restoreOnAppExit}
                disabled={busy}
                onChange={(e) => persist({ ...settings, restoreOnAppExit: e.target.checked })}
              />
            </label>
          </div>
        </div>
      )}

      {editorOpen && (
        <ModeEditorDialog
          displays={displays}
          initial={editing}
          defaultTarget={currentTarget()}
          onClose={() => setEditorOpen(false)}
          onSaved={() => load()}
        />
      )}
    </section>
  );
}
