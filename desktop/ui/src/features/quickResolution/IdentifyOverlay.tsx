// 识别叠层（display.md §8.2）：无边框/置顶/点击穿透窗体中央的巨大编号。
// URL 参数：?view=qr-identify&n=<index>

export default function IdentifyOverlay() {
  const n = new URLSearchParams(window.location.search).get("n") ?? "?";
  return (
    <div className="qr-identify-root">
      <div className="qr-identify-num">{n}</div>
    </div>
  );
}
