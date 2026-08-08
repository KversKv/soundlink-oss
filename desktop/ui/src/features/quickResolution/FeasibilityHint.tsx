// 可行性预检可视化（display.md §10.2 底部区域）。

import type { ValidationReport } from "./types";

interface Props {
  report: ValidationReport | null;
  loading: boolean;
}

export default function FeasibilityHint({ report, loading }: Props) {
  if (loading) {
    return <div className="qr-feasibility qr-feas-muted">计算中…</div>;
  }
  if (!report) {
    return null;
  }
  const pixGhz = (report.pixelClockKhz / 1e6).toFixed(3);
  return (
    <div className="qr-feasibility">
      <div className="qr-feas-line qr-feas-muted">
        像素时钟 {pixGhz} GPix/s
        {report.feasibility && (
          <>
            {"  |  "}未压缩需 {report.feasibility.requiredUncompressedGbps.toFixed(1)} Gbps
            {" / 可用 "}{report.feasibility.availableGbps.toFixed(1)}
          </>
        )}
      </div>
      {report.errors.length > 0 ? (
        <div className="qr-feas-line qr-feas-bad">
          ✕ {report.errors.join("；")}
        </div>
      ) : (
        <>
          <div className={`qr-feas-line ${report.feasibility?.dscOk === false ? "qr-feas-bad" : "qr-feas-good"}`}>
            {report.feasibility?.dscOk === true
              ? `✓ 可行（DSC 启用，压缩后约 ${report.feasibility.requiredDscGbps?.toFixed(1)} Gbps）`
              : report.inSystemList
                ? "✓ 已在系统模式列表中，可直接快切"
                : "✓ 参数有效"}
          </div>
          {!report.inSystemList && report.ok && (
            <div className="qr-feas-line qr-feas-warn">
              ⚠ 该模式不在系统列表中，保存后需执行「预置」：
              将注入 EDID（已自动备份）并重启显示驱动，约 3 秒黑屏。
            </div>
          )}
        </>
      )}
    </div>
  );
}
