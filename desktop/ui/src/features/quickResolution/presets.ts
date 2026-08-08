// 内置预设库（display.md §10.2）。

export interface ResolutionPreset {
  w: number;
  h: number;
  label: string;
}

export interface RatioGroup {
  id: string;
  label: string;
  ratio: number; // w/h
  presets: ResolutionPreset[];
}

export const RATIO_GROUPS: RatioGroup[] = [
  {
    id: "16:9",
    label: "16:9",
    ratio: 16 / 9,
    presets: [
      { w: 1280, h: 720, label: "1280×720" },
      { w: 1600, h: 900, label: "1600×900" },
      { w: 1920, h: 1080, label: "1920×1080" },
      { w: 2560, h: 1440, label: "2560×1440" },
      { w: 3200, h: 1800, label: "3200×1800" },
      { w: 3840, h: 2160, label: "3840×2160" },
      { w: 5120, h: 2880, label: "5120×2880" },
      { w: 7680, h: 4320, label: "7680×4320" },
    ],
  },
  {
    id: "16:10",
    label: "16:10",
    ratio: 16 / 10,
    presets: [
      { w: 1280, h: 800, label: "1280×800" },
      { w: 1680, h: 1050, label: "1680×1050" },
      { w: 1920, h: 1200, label: "1920×1200" },
      { w: 2560, h: 1600, label: "2560×1600" },
      { w: 3840, h: 2400, label: "3840×2400" },
    ],
  },
  {
    id: "4:3",
    label: "4:3",
    ratio: 4 / 3,
    presets: [
      { w: 1024, h: 768, label: "1024×768" },
      { w: 1280, h: 960, label: "1280×960" },
      { w: 1400, h: 1050, label: "1400×1050" },
      { w: 1440, h: 1080, label: "1440×1080" },
      { w: 1600, h: 1200, label: "1600×1200" },
      { w: 1920, h: 1440, label: "1920×1440" },
      { w: 2048, h: 1536, label: "2048×1536" },
    ],
  },
  {
    id: "21:9",
    label: "21:9",
    ratio: 21 / 9,
    presets: [
      { w: 2560, h: 1080, label: "2560×1080" },
      { w: 3440, h: 1440, label: "3440×1440" },
      { w: 3840, h: 1600, label: "3840×1600" },
    ],
  },
  {
    id: "32:9",
    label: "32:9",
    ratio: 32 / 9,
    presets: [
      { w: 3840, h: 1080, label: "3840×1080" },
      { w: 5120, h: 1440, label: "5120×1440" },
    ],
  },
];

export const REFRESH_QUICK: number[] = [60, 120, 144, 165, 240, 360, 480, 540];

/// 宽高比最接近的分组（容差 2%）。
export function matchRatioGroup(w: number, h: number): RatioGroup | null {
  if (h === 0) return null;
  const r = w / h;
  let best: RatioGroup | null = null;
  let bestDiff = Infinity;
  for (const g of RATIO_GROUPS) {
    const diff = Math.abs(r - g.ratio) / g.ratio;
    if (diff < bestDiff) {
      bestDiff = diff;
      best = g;
    }
  }
  return bestDiff <= 0.02 ? best : null;
}
