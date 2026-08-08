// 添加/编辑分辨率弹窗（display.md §10.2）。

import { useEffect, useMemo, useState } from "react";
import { qrApi } from "./api";
import FeasibilityHint from "./FeasibilityHint";
import { matchRatioGroup, RATIO_GROUPS, REFRESH_QUICK } from "./presets";
import type {
  ColorFormat,
  DisplayInfo,
  DisplayModeEntry,
  ModeTarget,
  ScalingMode,
  TimingStandard,
  ValidationReport,
} from "./types";

interface Props {
  displays: DisplayInfo[];
  /** 编辑已有模式时传入；新建为 null。 */
  initial: DisplayModeEntry | null;
  defaultTarget: ModeTarget;
  onClose: () => void;
  onSaved: (m: DisplayModeEntry) => void;
}

function emptyEntry(target: ModeTarget): DisplayModeEntry {
  return {
    id: "",
    label: "",
    width: 1920,
    height: 1080,
    refreshHz: 144,
    target,
    timingStandard: "auto",
    state: "draft",
    pinnedToTray: false,
    order: 0,
    createdAt: 0,
  };
}

export default function ModeEditorDialog({ displays, initial, defaultTarget, onClose, onSaved }: Props) {
  const [entry, setEntry] = useState<DisplayModeEntry>(initial ?? emptyEntry(defaultTarget));
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [validating, setValidating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");

  const ratioGroup = useMemo(
    () => matchRatioGroup(entry.width, entry.height),
    [entry.width, entry.height],
  );

  // 防抖实时预检。
  useEffect(() => {
    if (entry.width <= 0 || entry.height <= 0) return;
    const t = setTimeout(async () => {
      setValidating(true);
      try {
        const r = await qrApi.validateMode(entry);
        setReport(r);
      } catch {
        setReport(null);
      } finally {
        setValidating(false);
      }
    }, 350);
    return () => clearTimeout(t);
  }, [entry]);

  const set = <K extends keyof DisplayModeEntry>(k: K, v: DisplayModeEntry[K]) =>
    setEntry((e) => ({ ...e, [k]: v }));

  const applyRatio = (groupId: string) => {
    const g = RATIO_GROUPS.find((x) => x.id === groupId);
    if (!g || g.presets.length === 0) return;
    // 保持当前高度，按新比例推宽度（就近 8 对齐）。
    const h = entry.height || g.presets[0].h;
    const w = Math.round((h * g.ratio) / 8) * 8;
    setEntry((e) => ({ ...e, width: w, height: h }));
  };

  const currentTargetIndex = (t: ModeTarget): number => {
    if (t.kind === "primary") return 0;
    if (t.kind === "index") return t.index;
    const d = displays.find((d) => d.key.instance_path === t.key.instance_path);
    return d?.index ?? 0;
  };

  const setTargetByIndex = (idx: number) => {
    if (idx === 0) {
      set("target", { kind: "primary" });
      return;
    }
    const d = displays.find((d) => d.index === idx);
    set("target", d ? { kind: "key", key: d.key } : { kind: "index", index: idx });
  };

  const save = async () => {
    setSaving(true);
    setSaveError("");
    try {
      const label =
        entry.label.trim() || `${entry.width}×${entry.height} @${entry.refreshHz}Hz`;
      const saved = await qrApi.upsertMode({ ...entry, label });
      onSaved(saved);
      onClose();
    } catch (e) {
      setSaveError(typeof e === "object" && e !== null && "code" in e
        ? JSON.stringify(e)
        : String(e));
    } finally {
      setSaving(false);
    }
  };

  const targetIdx = currentTargetIndex(entry.target);

  return (
    <div className="qr-dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="qr-dialog" role="dialog" aria-modal="true">
        <div className="qr-dialog-title">{initial ? "编辑分辨率" : "添加分辨率"}</div>

        <label className="field-shell">
          <span>名称</span>
          <input
            type="text"
            value={entry.label}
            placeholder={`${entry.width}×${entry.height} @${entry.refreshHz}Hz`}
            onChange={(e) => set("label", e.target.value)}
          />
        </label>

        <label className="field-shell">
          <span>目标显示器</span>
          <select value={targetIdx} onChange={(e) => setTargetByIndex(Number(e.target.value))}>
            <option value={0}>主显示器</option>
            {displays.map((d) => (
              <option key={d.index} value={d.index}>
                {d.index} · {d.friendlyName}
                {d.isPrimary ? "（主）" : ""}
              </option>
            ))}
          </select>
        </label>

        <div className="qr-ratio-row">
          <span className="qr-field-label">比例预设</span>
          <div className="qr-chip-row">
            {RATIO_GROUPS.map((g) => (
              <button
                key={g.id}
                type="button"
                className={`qr-chip ${ratioGroup?.id === g.id ? "qr-chip-active" : ""}`}
                onClick={() => applyRatio(g.id)}
              >
                {g.label}
              </button>
            ))}
          </div>
        </div>
        {ratioGroup && (
          <div className="qr-chip-row qr-chip-sub">
            {ratioGroup.presets.map((p) => (
              <button
                key={p.label}
                type="button"
                className={`qr-chip ${entry.width === p.w && entry.height === p.h ? "qr-chip-active" : ""}`}
                onClick={() => setEntry((e) => ({ ...e, width: p.w, height: p.h }))}
              >
                {p.label}
              </button>
            ))}
          </div>
        )}

        <div className="qr-dim-row">
          <label className="field-shell">
            <span>宽</span>
            <input
              type="number"
              min={640}
              max={16384}
              value={entry.width}
              onChange={(e) => set("width", Math.max(0, Number(e.target.value)))}
            />
          </label>
          <label className="field-shell">
            <span>高</span>
            <input
              type="number"
              min={480}
              max={16384}
              value={entry.height}
              onChange={(e) => set("height", Math.max(0, Number(e.target.value)))}
            />
          </label>
          <span className="qr-ratio-badge">{ratioGroup ? `${ratioGroup.label} ✓` : "自定义"}</span>
        </div>

        <div className="qr-dim-row">
          <label className="field-shell" style={{ flex: 1 }}>
            <span>刷新率（Hz）</span>
            <input
              type="number"
              min={24}
              max={1000}
              value={entry.refreshHz}
              onChange={(e) => set("refreshHz", Math.max(0, Math.round(Number(e.target.value))))}
            />
          </label>
        </div>
        <div className="qr-chip-row qr-chip-sub qr-chip-gap">
          {REFRESH_QUICK.map((r) => (
            <button
              key={r}
              type="button"
              className={`qr-chip ${entry.refreshHz === r ? "qr-chip-active" : ""}`}
              onClick={() => set("refreshHz", r)}
            >
              {r}
            </button>
          ))}
        </div>

        {/* 跳过切换确认（高危，红字警示） */}
        <label className="toggle-row qr-skip-confirm-row">
          <input
            type="checkbox"
            checked={entry.skipConfirm ?? false}
            onChange={(e) => set("skipConfirm", e.target.checked)}
          />
          <span>切换此模式时不弹 15 秒确认窗</span>
        </label>
        <div className="qr-skip-confirm-warn">
          ⚠ 高危：勾选后切换即永久生效，无法反悔。请先确认该模式在你的显示器上可正常显示，再勾选。
        </div>

        <details className="qr-advanced" open={advancedOpen} onToggle={(e) => setAdvancedOpen((e.currentTarget as HTMLDetailsElement).open)}>
          <summary>高级</summary>
          <div className="qr-advanced-grid">
            <label className="field-shell">
              <span>色深</span>
              <select
                value={entry.bitDepth ?? 8}
                onChange={(e) => set("bitDepth", Number(e.target.value) as 8 | 10 | 12)}
              >
                <option value={8}>8 bpc</option>
                <option value={10}>10 bpc</option>
                <option value={12}>12 bpc</option>
              </select>
            </label>
            <label className="field-shell">
              <span>格式</span>
              <select
                value={entry.colorFormat ?? "RGB"}
                onChange={(e) => set("colorFormat", e.target.value as ColorFormat)}
              >
                <option value="RGB">RGB</option>
                <option value="YCbCr444">YCbCr 4:4:4</option>
                <option value="YCbCr422">YCbCr 4:2:2</option>
                <option value="YCbCr420">YCbCr 4:2:0</option>
              </select>
            </label>
            <label className="field-shell">
              <span>缩放</span>
              <select
                value={entry.scaling ?? "aspect"}
                onChange={(e) => set("scaling", e.target.value as ScalingMode)}
              >
                <option value="aspect">保持比例</option>
                <option value="fullscreen">全屏拉伸</option>
                <option value="centered">居中</option>
                <option value="noscaling">不缩放</option>
              </select>
            </label>
            <label className="field-shell">
              <span>时序标准</span>
              <select
                value={entry.timingStandard}
                onChange={(e) => set("timingStandard", e.target.value as TimingStandard)}
              >
                <option value="auto">自动（继承原生消隐）</option>
                <option value="cvt-rb2">CVT-RB2</option>
                <option value="cvt-rb3">CVT-RB3（极限高刷）</option>
              </select>
            </label>
          </div>
        </details>

        <div className="qr-feas-divider">─── 可行性预检 ───</div>
        <FeasibilityHint report={report} loading={validating} />

        {saveError && <div className="qr-feas-line qr-feas-bad">保存失败：{saveError}</div>}

        <div className="qr-dialog-actions">
          <button type="button" className="text-button" onClick={onClose} disabled={saving}>
            取消
          </button>
          <button
            type="button"
            className="primary-button"
            onClick={save}
            disabled={saving || (report !== null && !report.ok)}
          >
            {saving ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}
