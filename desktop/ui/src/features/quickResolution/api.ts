// QR-1 IPC 封装（display.md §10.3）。

import { invoke } from "@tauri-apps/api/core";
import type {
  BackupInfo,
  DisplayInfo,
  DisplayModeEntry,
  ModeTarget,
  ProvisionReport,
  QrAvailability,
  QuickResolutionSettings,
  SwitchResult,
  ValidationReport,
} from "./types";

export const qrApi = {
  getAvailability: () => invoke<QrAvailability>("qr_get_availability"),
  getDisplays: () => invoke<DisplayInfo[]>("qr_get_displays"),
  identifyDisplays: () => invoke<void>("qr_identify_displays"),
  getSettings: () => invoke<QuickResolutionSettings>("qr_get_settings"),
  setSettings: (settings: QuickResolutionSettings) =>
    invoke<QuickResolutionSettings>("qr_set_settings", { settings }),
  listModes: () => invoke<DisplayModeEntry[]>("qr_list_modes"),
  upsertMode: (entry: DisplayModeEntry) =>
    invoke<DisplayModeEntry>("qr_upsert_mode", { entry }),
  deleteMode: (id: string) => invoke<void>("qr_delete_mode", { id }),
  reorderModes: (ids: string[]) => invoke<void>("qr_reorder_modes", { ids }),
  importSystemModes: (target: ModeTarget) =>
    invoke<DisplayModeEntry[]>("qr_import_system_modes", { target }),
  validateMode: (draft: DisplayModeEntry) =>
    invoke<ValidationReport>("qr_validate_mode", { draft }),
  apply: (id: string) => invoke<SwitchResult>("qr_apply", { id }),
  applyPrevious: () => invoke<SwitchResult>("qr_apply_previous"),
  confirmApply: () => invoke<void>("qr_confirm_apply"),
  revertApply: () => invoke<void>("qr_revert_apply"),
  listEdidBackups: (target?: ModeTarget) =>
    invoke<BackupInfo[]>("qr_list_edid_backups", { target: target ?? null }),
  refreshStates: () => invoke<void>("qr_refresh_states"),
  // M7 预置（后端在 M7 实现；前端预置按钮届时接入）。
  provision: (ids: string[]) => invoke<ProvisionReport>("qr_provision", { ids }),
};
