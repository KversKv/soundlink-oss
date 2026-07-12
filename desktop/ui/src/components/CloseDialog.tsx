import { useState } from "react";

interface Props {
  onClose: () => void;
  onMinimize: (remember: boolean) => Promise<void>;
  onQuit: (remember: boolean) => Promise<void>;
}

export default function CloseDialog({ onClose, onMinimize, onQuit }: Props) {
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);

  const wrap = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dialog-overlay" role="dialog" aria-modal="true">
      <div className="dialog-card">
        <h3>关闭窗口时你想要？</h3>
        <div className="dialog-actions">
          <button disabled={busy} onClick={() => wrap(() => onMinimize(remember))}>
            最小化到托盘
          </button>
          <button disabled={busy} onClick={() => wrap(() => onQuit(remember))}>
            退出程序
          </button>
          <button disabled={busy} onClick={onClose}>
            取消
          </button>
        </div>
        <label className="remember-row">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
          />
          <span>记住我的选择</span>
        </label>
      </div>
    </div>
  );
}
