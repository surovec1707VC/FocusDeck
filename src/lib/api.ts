import { invoke } from "@tauri-apps/api/core";

/** Error shape returned by Rust commands (`AppError`). */
export interface AppError {
  kind: string;
  message: string;
}

export class ApiError extends Error {
  constructor(public kind: string, message: string) {
    super(message);
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "message" in e) {
      const err = e as AppError;
      throw new ApiError(err.kind ?? "internal", err.message);
    }
    throw new ApiError("internal", String(e));
  }
}

export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

// --- types (mirror the Rust structs) ---

export type Theme = "dark" | "light" | "system";

export interface AppSettings {
  theme: Theme;
  language: string;
  close_to_tray: boolean;
  start_minimized: boolean;
  autostart: boolean;
  global_hotkeys_enabled: boolean;
  toggle_window_hotkey: string;
}

export interface AppInfo {
  name: string;
  version: string;
  tauri_version: string;
  data_dir: string;
  log_dir: string;
  db_file: string;
}

export interface DashboardStats {
  modules_enabled: number;
  modules_total: number;
  automations_total: number;
  automations_enabled: number;
  layouts: number;
  active_layout: string | null;
  widgets_active: number;
  tasks_open: number;
  permissions_granted: number;
  permissions_total: number;
  hotkeys_registered: number;
  autostart: boolean;
}

export interface Permission {
  id: string;
  name: string;
  description: string;
  granted: boolean;
  updated_at: string;
}

export interface PermissionLog {
  id: number;
  permission_id: string;
  action: "granted" | "revoked" | "denied";
  source: string;
  created_at: string;
}

export interface ModuleInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  enabled: boolean;
  is_core: boolean;
  permissions: string[];
  missing_permissions: string[];
}

export interface Layout {
  id: string;
  name: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export type WidgetKind = "clock" | "note" | "tasks" | "status";
export type WidgetSize = "sm" | "md" | "lg";

export interface Widget {
  id: string;
  layout_id: string;
  kind: WidgetKind;
  title: string;
  size: WidgetSize;
  position: number;
  config: Record<string, unknown>;
}

export interface Task {
  id: string;
  title: string;
  done: boolean;
  position: number;
  created_at: string;
}

export type Trigger =
  | { type: "app_start" }
  | { type: "interval"; minutes: number }
  | { type: "hotkey"; shortcut: string };

export type Action =
  | { type: "notify"; title: string; body: string }
  | { type: "log"; message: string }
  | { type: "show_window" }
  | { type: "hide_window" };

export interface Automation {
  id: string;
  name: string;
  enabled: boolean;
  trigger: Trigger;
  actions: Action[];
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface AutomationInput {
  name: string;
  enabled: boolean;
  trigger: Trigger;
  actions: Action[];
}

export interface AutomationLog {
  id: number;
  automation_id: string;
  status: "ok" | "error" | "skipped";
  message: string;
  created_at: string;
}

export interface Notification {
  title: string;
  body: string;
  source: string;
}

// --- commands ---

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  dashboardStats: () => call<DashboardStats>("dashboard_stats"),

  settingsGet: () => call<AppSettings>("settings_get"),
  settingsUpdate: (settings: AppSettings) => call<AppSettings>("settings_update", { settings }),

  permissionsList: () => call<Permission[]>("permissions_list"),
  permissionsSet: (id: string, granted: boolean) => call<Permission[]>("permissions_set", { id, granted }),
  permissionsLogs: (limit = 50) => call<PermissionLog[]>("permissions_logs", { limit }),

  modulesList: () => call<ModuleInfo[]>("modules_list"),
  modulesSetEnabled: (id: string, enabled: boolean) => call<ModuleInfo[]>("modules_set_enabled", { id, enabled }),

  layoutsList: () => call<Layout[]>("layouts_list"),
  layoutsCreate: (name: string) => call<Layout[]>("layouts_create", { name }),
  layoutsRename: (id: string, name: string) => call<Layout[]>("layouts_rename", { id, name }),
  layoutsActivate: (id: string) => call<Layout[]>("layouts_activate", { id }),
  layoutsDelete: (id: string) => call<Layout[]>("layouts_delete", { id }),

  widgetsList: (layoutId?: string) => call<Widget[]>("widgets_list", { layoutId }),
  widgetsAdd: (kind: WidgetKind, title: string, size: WidgetSize) => call<Widget[]>("widgets_add", { kind, title, size }),
  widgetsUpdate: (id: string, patch: { title?: string; size?: WidgetSize; config?: Record<string, unknown> }) =>
    call<Widget>("widgets_update", { id, patch }),
  widgetsMove: (id: string, delta: number) => call<void>("widgets_move", { id, delta }),
  widgetsRemove: (id: string) => call<void>("widgets_remove", { id }),

  tasksList: () => call<Task[]>("tasks_list"),
  tasksAdd: (title: string) => call<Task[]>("tasks_add", { title }),
  tasksToggle: (id: string) => call<Task[]>("tasks_toggle", { id }),
  tasksDelete: (id: string) => call<Task[]>("tasks_delete", { id }),

  automationsList: () => call<Automation[]>("automations_list"),
  automationsCreate: (input: AutomationInput) => call<Automation>("automations_create", { input }),
  automationsUpdate: (id: string, input: AutomationInput) => call<Automation>("automations_update", { id, input }),
  automationsSetEnabled: (id: string, enabled: boolean) => call<Automation>("automations_set_enabled", { id, enabled }),
  automationsDelete: (id: string) => call<void>("automations_delete", { id }),
  automationsRunNow: (id: string) => call<void>("automations_run_now", { id }),
  automationsLogs: (id?: string, limit = 50) => call<AutomationLog[]>("automations_logs", { id, limit }),
};

export const EVENTS = {
  notify: "focusdeck://notify",
  navigate: "focusdeck://navigate",
} as const;
