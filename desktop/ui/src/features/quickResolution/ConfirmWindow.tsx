// 15 秒切换确认窗（独立置顶小窗 `qr-confirm`，display.md §7.4）。
// URL 参数：?view=qr-confirm&mode=<text>&timeout=<secs>&display=<index>

import { useEffect, useState } from "react";
import { qrApi } from "./api";

export default function ConfirmWindow() {
  const params = new URLSearchParams(window.location.search);
  const modeText = params.get("mode") ?? "";
  const timeout = Math.max(1, Number(params.get("timeout") ?? 15));
  const displayIndex = params.get("display") ?? "";
  const [left, setLeft] = useState(timeout);
  const [done, setDone] = useState(false);

  useEffect(() => {
    if (done) return;
    const id = setInterval(() => {
      setLeft((s) => {
        if (s <= 1) {
          clearInterval(id);
          return 0;
        }
        return s - 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [done]);

  const confirm = async () => {
    setDone(true);
    try {
      await qrApi.confirmApply();
    } finally {
      window.close();
    }
  };

  const revert = async () => {
    setDone(true);
    try {
      await qrApi.revertApply();
    } finally {
      window.close();
    }
  };

  return (
    <div className="qr-confirm-root">
      <div className="qr-confirm-title">分辨率已切换</div>
      <div className="qr-confirm-mode">
        显示器 {displayIndex} · {modeText}
      </div>
      <div className="qr-confirm-count">
        <strong>{left}</strong> 秒后自动回滚
      </div>
      <div className="qr-confirm-actions">
        <button type="button" className="primary-button" onClick={confirm} disabled={done}>
          保留此设置
        </button>
        <button type="button" className="text-button" onClick={revert} disabled={done}>
          立即回滚
        </button>
      </div>
    </div>
  );
}
