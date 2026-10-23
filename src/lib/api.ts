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

export type PermissionCategory = "files" | "clipboard" | "windows" | "keyboard" | "mouse" | "screen" | "processes" | "network" | "notifications" | "system";

export interface PermissionInfo extends Permission {
  category: PermissionCategory;
  used_by: { id: string; name: string; enabled: boolean }[];
  uses: number;
  last_used_at: string | null;
  denials: number;
}

export interface AccessEntry {
  created_at: string;
  kind: "granted" | "revoked" | "denied" | "used" | "blocked" | "error" | "cancelled" | "undone";
  source: string;
  detail: string;
}

export type Risk = "read" | "change" | "dangerous";

export interface SafetyAction {
  id: string;
  name: string;
  module: string;
  permission: string | null;
  risk: Risk;
  undoable: boolean;
}

export interface SafeMode {
  enabled: boolean;
  whitelist: string[];
}

export interface ActionLogEntry {
  id: number;
  created_at: string;
  module_id: string;
  action: string;
  target: string;
  result: "ok" | "error" | "denied" | "blocked" | "cancelled" | "undone";
  message: string;
  permission: string | null;
  can_undo: boolean;
  undone_at: string | null;
}

export interface WindowInfo {
  hwnd: number;
  title: string;
  process: string;
  minimized: boolean;
  maximized: boolean;
  active: boolean;
}

export interface InstalledApp {
  name: string;
  app_id: string;
}

export interface PermissionLog {
  id: number;
  permission_id: string;
  action: "granted" | "revoked" | "denied";
  source: string;
  created_at: string;
}

export type ModuleCategory =
  | "appearance"
  | "widgets"
  | "layouts"
  | "automations"
  | "productivity"
  | "developer_tools"
  | "clipboard"
  | "files"
  | "windows_integration"
  | "notifications"
  | "privacy_security"
  | "integrations"
  | "performance";

export interface SchemaProperty {
  type?: "boolean" | "integer" | "number" | "string";
  title?: string;
  description?: string;
  default?: unknown;
  minimum?: number;
  maximum?: number;
  enum?: unknown[];
}

export interface SettingsSchema {
  type: "object";
  properties?: Record<string, SchemaProperty>;
}

export type Settings = Record<string, unknown>;

export interface ModulePermission {
  id: string;
  name: string;
  granted: boolean;
}

export interface ModuleInfo {
  id: string;
  name: string;
  description: string;
  details: string;
  category: ModuleCategory;
  version: string;
  enabled: boolean;
  critical: boolean;
  requires_restart: boolean;
  pending_restart: boolean;
  permissions: ModulePermission[];
  missing_permissions: string[];
  settings: Settings;
  settings_schema: SettingsSchema;
  updated_at: string;
}

export type HealthState = "ok" | "degraded" | "error" | "disabled";

export interface ModuleStatus {
  module_id: string;
  enabled: boolean;
  state: HealthState;
  message: string;
  pending_restart: boolean;
  checked_at: string;
}

export interface ModuleLog {
  id: number;
  module_id: string;
  level: "info" | "warn" | "error";
  action: string;
  message: string;
  source: string;
  created_at: string;
}

export interface ToggleResult {
  module: ModuleInfo;
  warnings: string[];
}

export interface ImportReport {
  settings_applied: boolean;
  modules_applied: number;
  permissions_applied: number;
  skipped: string[];
  warnings: string[];
}

export type LayoutProfile = "work" | "coding" | "streaming" | "gaming" | "focus" | "custom";

export interface Layout {
  id: string;
  name: string;
  profile: LayoutProfile;
  /** Global shortcut that switches to this layout. */
  hotkey: string | null;
  is_active: boolean;
  position: number;
  widget_count: number;
  created_at: string;
  updated_at: string;
}

export type WidgetKind =
  | "clock"
  | "date"
  | "calendar"
  | "countdown"
  | "stopwatch"
  | "notes"
  | "todo"
  | "quick_links"
  | "app_launcher"
  | "system_info"
  | "clipboard"
  | "notifications"
  | "weather"
  | "status"
  | "git";

export type WidgetCategory = "time" | "productivity" | "system" | "information";
export type DataSource = "local" | "database" | "config" | "system" | "clipboard" | "network";

/** A widget on a layout. Position and size are in grid cells. */
export interface Widget {
  id: string;
  layout_id: string;
  kind: WidgetKind;
  title: string;
  position: number;
  config: Record<string, unknown>;
  x: number;
  y: number;
  w: number;
  h: number;
  pinned: boolean;
  collapsed: boolean;
  hidden: boolean;
}

export interface WidgetPatch {
  title?: string;
  config?: Record<string, unknown>;
  pinned?: boolean;
  collapsed?: boolean;
  hidden?: boolean;
}

export interface GridItem {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A widget kind and the state of its module (Widgets page). */
export interface WidgetCatalogEntry {
  kind: WidgetKind;
  title: string;
  module_id: string;
  name: string;
  description: string;
  details: string;
  category: WidgetCategory;
  data_source: DataSource;
  default_size: { w: number; h: number };
  min_size: { w: number; h: number };
  refresh_secs: number;
  enabled: boolean;
  available: boolean;
  missing_modules: string[];
  permissions: ModulePermission[];
  missing_permissions: string[];
  settings: Settings;
  settings_schema: SettingsSchema;
  instances: number;
}

export interface WidgetData<T = any> {
  widget_id: string;
  kind: WidgetKind;
  data: T;
  fetched_at: string | null;
  from_cache: boolean;
  /** Why fresh data isn't available; `data` may still hold cached data. */
  error: string | null;
  refresh_secs: number;
}

export interface PickedApp {
  name: string;
  path: string;
}

export type TaskStatus = "todo" | "in_progress" | "done" | "cancelled";
export type Recurrence = "none" | "daily" | "weekdays" | "weekly" | "monthly";

export interface Task {
  id: string;
  title: string;
  done: boolean;
  status: TaskStatus;
  /** 0 none … 3 high */
  priority: number;
  /** YYYY-MM-DD */
  due_at: string | null;
  /** "Работа/Отчёты" */
  category: string;
  recurrence: Recurrence;
  notes: string;
  position: number;
  created_at: string;
  completed_at: string | null;
}

export interface TaskFilter {
  status?: "open" | "all" | TaskStatus;
  priority?: number;
  category?: string;
  due?: "today" | "overdue" | "week" | "none";
  search?: string;
}

export interface TaskInput {
  title?: string;
  status?: TaskStatus;
  priority?: number;
  due_at?: string | null;
  category?: string;
  recurrence?: Recurrence;
  notes?: string;
}

export interface TaskStats {
  completed_today: number;
  created_today: number;
  open: number;
  in_progress: number;
  overdue: number;
  due_today: number;
  by_priority: [number, number, number, number];
  last_7_days: { date: string; count: number }[];
  categories: string[];
}

export interface QuickTask {
  title: string;
  priority: number | null;
  due: string | null;
  category: string | null;
  recurrence: Recurrence | null;
}

export interface Note {
  id: string;
  title: string;
  body: string;
  pinned: boolean;
  tags: string[];
  project: string;
  layout_id: string | null;
  deleted_at: string | null;
  /** Belongs to a dashboard widget. */
  widget: boolean;
  created_at: string;
  updated_at: string;
}

export interface NoteFilter {
  search?: string;
  tag?: string;
  project?: string;
  layout_id?: string;
  trash?: boolean;
}

export interface NoteInput {
  title?: string;
  body?: string;
  tags?: string[];
  pinned?: boolean;
  project?: string;
  layout_id?: string | null;
}

export interface NoteFacets {
  tags: [string, number][];
  projects: string[];
  trash: number;
}

export interface Snippet {
  id: string;
  title: string;
  body: string;
  shortcut: string | null;
  tags: string[];
  /** Variables to fill in (built-ins excluded). */
  variables: string[];
  use_count: number;
  updated_at: string;
}

export interface SnippetInput {
  title: string;
  body: string;
  shortcut?: string | null;
  tags: string[];
}

export interface SnippetImportReport {
  imported: number;
  duplicates: number;
  skipped: string[];
}

export type ClipKind = "text" | "link" | "files" | "image" | "html" | "rtf";

export interface ClipItem {
  id: number;
  kind: ClipKind;
  /** Readable text: the text, a file list or an image summary. */
  content: string;
  pinned: boolean;
  source_app: string;
  size: number;
  image_width: number | null;
  image_height: number | null;
  files_json: string | null;
  has_html: boolean;
  has_rtf: boolean;
  created_at: string;
  last_used_at: string | null;
}

export interface ClipFilter {
  search?: string;
  kind?: ClipKind;
  pinned_only?: boolean;
  limit?: number;
  offset?: number;
}

export interface ClipDetails {
  text: string | null;
  html_text: string | null;
  rtf: string | null;
  files: string[] | null;
}

export interface PrivateMode {
  active: boolean;
  /** Epoch ms when it turns itself off. */
  until: number | null;
}

export interface HubStatus {
  recording: boolean;
  permission: boolean;
  private_mode: PrivateMode;
  stats: { total: number; pinned: number; bytes: number; by_kind: [ClipKind, number][] };
}

export type QuickActionKind = "open_app" | "open_url" | "open_folder" | "open_file" | "paste_text" | "copy_text" | "command";

export interface QuickAction {
  id: string;
  name: string;
  kind: QuickActionKind;
  target: string;
  hotkey: string | null;
  position: number;
  last_run_at: string | null;
}

export interface QuickActionInput {
  name: string;
  kind: QuickActionKind;
  target: string;
  hotkey?: string | null;
}

export interface CommandOutput {
  code: number | null;
  stdout: string;
  stderr: string;
  timed_out: boolean;
}

export type FileCategory = "screenshots" | "images" | "documents" | "archives" | "installers" | "video" | "music" | "code" | "other";

export interface OrganizerMatch {
  categories?: FileCategory[];
  extensions?: string[];
  name_pattern?: string;
  older_than_days?: number | null;
  newer_than_days?: number | null;
  min_size_kb?: number | null;
  max_size_kb?: number | null;
}

export interface OrganizerRule {
  id: string;
  name: string;
  enabled: boolean;
  position: number;
  match: OrganizerMatch;
  /** Folder template: {root} {category} {ext} {year} {month} {day} */
  target: string;
  builtin: string | null;
}

export interface OrganizerRuleInput {
  name: string;
  enabled: boolean;
  match: OrganizerMatch;
  target: string;
}

export interface PlanItem {
  path: string;
  name: string;
  size: number;
  modified: string;
  category: FileCategory;
  rule_id: string;
  rule_name: string;
  target_dir: string;
}

export interface OrganizerPlan {
  desktop: string;
  desktop_files: number;
  items: PlanItem[];
  skipped: { name: string; reason: string }[];
  targets: [string, number][];
}

export interface OrganizerOperation {
  id: string;
  source: string;
  status: "done" | "partial" | "undone" | "partially_undone";
  moved_count: number;
  failed_count: number;
  summary: string;
  created_at: string;
  undone_at: string | null;
}

export interface OrganizerMove {
  id: number;
  operation_id: string;
  original_path: string;
  new_path: string;
  rule_name: string;
  size: number;
  status: "moved" | "failed" | "undone" | "undo_failed";
  error: string;
  moved_at: string;
  undone_at: string | null;
  restored_path: string | null;
}

export interface ApplyReport {
  operation_id: string;
  moved: number;
  failed: number;
  errors: string[];
}

export interface UndoReport {
  restored: number;
  failed: number;
  renamed: number;
  errors: string[];
}

export type TimerPhase = "work" | "short_break" | "long_break";

export interface Timer {
  id: string;
  name: string;
  kind: "countdown" | "pomodoro";
  duration_ms: number;
  work_ms: number;
  short_break_ms: number;
  long_break_ms: number;
  rounds: number;
  phase: TimerPhase;
  round: number;
  running: boolean;
  ends_at: number | null;
  remaining_ms: number | null;
  phase_started_at: number | null;
}

export interface TimerInput {
  name: string;
  kind: "countdown" | "pomodoro";
  duration_ms?: number;
  work_ms?: number;
  short_break_ms?: number;
  long_break_ms?: number;
  rounds?: number;
}

export interface Stopwatch {
  id: string;
  name: string;
  elapsed_ms: number;
  started_at: number | null;
  laps: number[];
}

export interface TimersState {
  timers: Timer[];
  stopwatches: Stopwatch[];
  stats: { pomodoros_today: number; focus_ms_today: number; sessions_today: number };
  now_ms: number;
}

export interface TimerSession {
  id: number;
  timer_id: string | null;
  timer_name: string;
  kind: "countdown" | TimerPhase;
  planned_ms: number;
  actual_ms: number;
  completed: boolean;
  started_at: string;
  ended_at: string;
}

export interface StopwatchResult {
  id: number;
  stopwatch_id: string | null;
  name: string;
  total_ms: number;
  laps_json: string;
  note: string;
  saved_at: string;
}

export type ChangeKind = "any" | "up" | "down";

export type Trigger =
  | { type: "app_start" }
  | { type: "system_start"; within_minutes?: number }
  | { type: "process_started"; process: string }
  | { type: "process_stopped"; process: string }
  | { type: "window_focus"; process?: string; title_contains?: string }
  | { type: "hotkey"; shortcut: string }
  | { type: "time"; time: string; days?: number[] }
  | { type: "interval"; minutes: number }
  | { type: "display_change"; event?: ChangeKind }
  | { type: "network_change"; event?: ChangeKind }
  | { type: "manual" }
  | { type: "ui_button"; label?: string };

export type Condition =
  | { type: "process_running"; process: string; running?: boolean }
  | { type: "active_app"; process: string }
  | { type: "weekday"; days: number[] }
  | { type: "time_range"; from: string; to: string }
  | { type: "module_enabled"; module_id: string; enabled?: boolean }
  | { type: "active_layout"; layout_id: string }
  | { type: "internet"; online?: boolean };

export type Action =
  | { type: "open_app"; path: string; args?: string }
  | { type: "close_app"; process: string; force?: boolean }
  | { type: "open_url"; url: string }
  | { type: "open_folder"; path: string }
  | { type: "open_file"; path: string }
  | { type: "run_command"; command: string }
  | { type: "switch_layout"; layout_id?: string; profile?: string }
  | { type: "set_module"; module_id: string; enabled: boolean }
  | { type: "notify"; title: string; body?: string }
  | { type: "copy_text"; text: string }
  | { type: "paste_text"; text: string }
  | { type: "screenshot"; folder?: string }
  | { type: "start_timer"; name: string; kind?: "countdown" | "pomodoro"; minutes?: number }
  | { type: "focus_mode"; enabled: boolean; minutes?: number }
  | { type: "delete_files"; folder: string; pattern?: string; older_than_days?: number }
  | { type: "log"; message: string }
  | { type: "show_window" }
  | { type: "hide_window" };

export interface Automation {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  module_id: string;
  trigger: Trigger;
  conditions: Condition[];
  actions: Action[];
  cooldown_secs: number;
  template: string | null;
  /** Permissions the actions need. */
  permissions: string[];
  /** Dangerous actions: each run is confirmed. */
  dangerous: string[];
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface AutomationInput {
  name: string;
  description?: string;
  enabled: boolean;
  /** Owning module; the automation doesn't run while it is off (default "automations"). */
  module_id?: string;
  trigger: Trigger;
  conditions?: Condition[];
  actions: Action[];
  cooldown_secs?: number;
}

export interface AutomationLog {
  id: number;
  automation_id: string;
  status: "ok" | "error" | "skipped";
  message: string;
  source: string;
  duration_ms: number;
  details_json: string;
  created_at: string;
}

export interface RunReport {
  status: "ok" | "error" | "skipped" | "pending" | "dry_run";
  message: string;
  conditions: { ok: boolean; text: string }[];
  plan: string[];
  actions: { action: string; ok: boolean; message: string }[];
  missing_permissions: string[];
  dangers: string[];
}

export interface PendingRun {
  token: string;
  automation_id: string;
  name: string;
  source: string;
  dangers: string[];
  plan: string[];
}

export interface AutomationTemplate {
  key: string;
  name: string;
  description: string;
  icon: string;
  input: AutomationInput;
}

export interface FocusState {
  active: boolean;
  until: number | null;
}

export interface Notification {
  title: string;
  body: string;
  source: string;
}

// --- commands ---

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  sessionStatus: () => call<SessionStatus>("session_status"),
  sessionAck: () => call<void>("session_ack"),
  backupCreate: () => call<string | null>("backup_create"),
  backupRestore: (confirm = false) => call<string | null>("backup_restore", { confirm }),
  appRestart: () => call<void>("app_restart"),
  dashboardStats: () => call<DashboardStats>("dashboard_stats"),

  settingsGet: () => call<AppSettings>("settings_get"),
  settingsUpdate: (settings: AppSettings) => call<AppSettings>("settings_update", { settings }),

  permissionsList: () => call<Permission[]>("permissions_list"),
  permissionsSet: (id: string, granted: boolean) => call<Permission[]>("permissions_set", { id, granted }),
  permissionsLogs: (limit = 50) => call<PermissionLog[]>("permissions_logs", { limit }),
  permissionsCenter: () => call<PermissionInfo[]>("permissions_center"),
  permissionsAccessLog: (id: string, limit = 100) => call<AccessEntry[]>("permissions_access_log", { id, limit }),

  // Safety: Safe Mode and the Action Log (see docs/windows-integration.md)
  safetyActions: () => call<SafetyAction[]>("safety_actions"),
  safeModeGet: () => call<SafeMode>("safe_mode_get"),
  safeModeSet: (mode: SafeMode) => call<SafeMode>("safe_mode_set", { mode }),
  actionLogList: (module?: string, limit = 200) => call<ActionLogEntry[]>("action_log_list", { module, limit }),
  actionLogUndo: (id: number) => call<string>("action_log_undo", { id }),

  // Windows Integration
  winWindows: () => call<WindowInfo[]>("win_windows"),
  winActiveWindow: () => call<WindowInfo | null>("win_active_window"),
  winWindow: (hwnd: number, action: "focus" | "minimize" | "maximize" | "restore" | "close", confirm = false) => call<void>("win_window", { hwnd, action, confirm }),
  winApps: (query?: string, refresh = false) => call<InstalledApp[]>("win_apps", { query, refresh }),
  winLaunch: (appId: string, confirm = false) => call<void>("win_launch", { appId, confirm }),
  winDesktops: () => call<{ count: number; current: number | null }>("win_desktops"),
  winDesktop: (action: "next" | "previous" | "new" | "close", confirm = false) => call<void>("win_desktop", { action, confirm }),
  winVolume: () => call<{ level: number; muted: boolean }>("win_volume"),
  winSetVolume: (level?: number, muted?: boolean, confirm = false) => call<{ level: number; muted: boolean }>("win_set_volume", { level, muted, confirm }),
  winBrightness: () => call<number | null>("win_brightness"),
  winSetBrightness: (level: number, confirm = false) => call<void>("win_set_brightness", { level, confirm }),
  winPowerPlans: () => call<{ guid: string; name: string; active: boolean }[]>("win_power_plans"),
  winSetPowerPlan: (guid: string, confirm = false) => call<{ guid: string; name: string; active: boolean }[]>("win_set_power_plan", { guid, confirm }),
  winWifi: () => call<{ available: boolean; connected: boolean; adapter: string; ssid: string | null; signal: number | null }>("win_wifi"),
  winBluetooth: () => call<{ available: boolean; enabled: boolean; adapter: string; devices: string[] }>("win_bluetooth"),
  winScreenshot: (copy = false) => call<{ path: string; copied: boolean }>("win_screenshot", { copy }),
  winClipboardRead: () => call<string | null>("win_clipboard_read"),
  winClipboardWrite: (text: string, confirm = false) => call<void>("win_clipboard_write", { text, confirm }),
  winNotify: (title: string, body: string, native: boolean) => call<void>("win_notify", { title, body, native }),
  winUiaInspect: (hwnd: number) => call<{ depth: number; name: string; control_type: string; automation_id: string }[]>("win_uia_inspect", { hwnd }),

  // Module Registry (see docs/api.md)
  listModules: () => call<ModuleInfo[]>("list_modules"),
  getModule: (id: string) => call<ModuleInfo>("get_module", { id }),
  enableModule: (id: string) => call<ToggleResult>("enable_module", { id }),
  disableModule: (id: string, confirm = false) => call<ToggleResult>("disable_module", { id, confirm }),
  updateModuleSettings: (id: string, settings: Settings) => call<ModuleInfo>("update_module_settings", { id, settings }),
  resetModuleSettings: (id: string) => call<ModuleInfo>("reset_module_settings", { id }),
  getModuleStatus: (id: string) => call<ModuleStatus>("get_module_status", { id }),
  getModuleLogs: (id: string, limit = 100) => call<ModuleLog[]>("get_module_logs", { id, limit }),
  getModulePermissions: (id: string) => call<ModulePermission[]>("get_module_permissions", { id }),

  // Configuration export / import
  exportConfig: () => call<string>("export_config"),
  importConfig: (json: string, confirm = false) => call<ImportReport>("import_config", { json, confirm }),
  exportConfigToFile: () => call<string | null>("export_config_to_file"),
  readConfigFile: () => call<string | null>("read_config_file"),

  // Layouts (see docs/widgets.md)
  layoutsList: () => call<Layout[]>("layouts_list"),
  layoutsCreate: (name: string, profile: LayoutProfile = "custom") => call<Layout[]>("layouts_create", { name, profile }),
  layoutsRename: (id: string, name: string) => call<Layout[]>("layouts_rename", { id, name }),
  layoutsDuplicate: (id: string) => call<Layout[]>("layouts_duplicate", { id }),
  layoutsActivate: (id: string) => call<Layout[]>("layouts_activate", { id }),
  layoutsDelete: (id: string) => call<Layout[]>("layouts_delete", { id }),
  layoutsSetHotkey: (id: string, hotkey: string | null) => call<Layout[]>("layouts_set_hotkey", { id, hotkey }),

  // Widgets
  widgetCatalog: () => call<WidgetCatalogEntry[]>("widget_catalog"),
  widgetsList: (layoutId?: string) => call<Widget[]>("widgets_list", { layoutId }),
  widgetsAdd: (kind: WidgetKind, layoutId?: string, title?: string) => call<Widget>("widgets_add", { kind, layoutId, title }),
  widgetsUpdate: (id: string, patch: WidgetPatch) => call<Widget>("widgets_update", { id, patch }),
  widgetsSaveGrid: (layoutId: string, items: GridItem[]) => call<void>("widgets_save_grid", { layoutId, items }),
  widgetsRemove: (id: string) => call<void>("widgets_remove", { id }),
  widgetData: <T = any>(id: string, params?: Record<string, unknown>, force = false) =>
    call<WidgetData<T>>("widget_data", { id, params, force }),
  widgetAction: <T = any>(id: string, action: string, payload?: Record<string, unknown>, params?: Record<string, unknown>) =>
    call<WidgetData<T>>("widget_action", { id, action, payload, params }),
  widgetPickApp: () => call<PickedApp | null>("widget_pick_app"),

  tasksList: () => call<Task[]>("tasks_list"),
  tasksAdd: (title: string) => call<Task[]>("tasks_add", { title }),
  tasksToggle: (id: string) => call<Task[]>("tasks_toggle", { id }),
  tasksDelete: (id: string) => call<Task[]>("tasks_delete", { id }),

  // Notes
  notesList: (filter?: NoteFilter) => call<Note[]>("notes_list", { filter }),
  notesFacets: () => call<NoteFacets>("notes_facets"),
  notesCreate: (input: NoteInput) => call<Note>("notes_create", { input }),
  notesUpdate: (id: string, input: NoteInput) => call<Note>("notes_update", { id, input }),
  notesTrash: (id: string) => call<void>("notes_trash", { id }),
  notesRestore: (id: string) => call<void>("notes_restore", { id }),
  notesDeleteForever: (id: string) => call<void>("notes_delete_forever", { id }),
  notesEmptyTrash: () => call<number>("notes_empty_trash"),
  notesExport: (id: string, format: "md" | "txt") => call<string | null>("notes_export", { id, format }),

  // Tasks
  tasksQuery: (filter?: TaskFilter) => call<Task[]>("tasks_query", { filter }),
  tasksCreate: (input: TaskInput) => call<Task>("tasks_create", { input }),
  tasksQuickAdd: (line: string) => call<Task>("tasks_quick_add", { line }),
  tasksQuickParse: (line: string) => call<QuickTask>("tasks_quick_parse", { line }),
  tasksUpdate: (id: string, input: TaskInput) => call<Task>("tasks_update", { id, input }),
  tasksRemove: (id: string) => call<void>("tasks_remove", { id }),
  tasksStats: () => call<TaskStats>("tasks_stats"),

  // Snippets
  snippetsList: (search?: string, tag?: string) => call<Snippet[]>("snippets_list", { search, tag }),
  snippetsSave: (input: SnippetInput, id?: string) => call<Snippet>("snippets_save", { id, input }),
  snippetsDelete: (id: string) => call<void>("snippets_delete", { id }),
  snippetsRender: (id: string, values?: Record<string, string>) => call<string>("snippets_render", { id, values }),
  snippetsCopy: (id: string, values?: Record<string, string>) => call<string>("snippets_copy", { id, values }),
  snippetsPaste: (id: string, values?: Record<string, string>) => call<void>("snippets_paste", { id, values }),
  snippetsExport: () => call<string | null>("snippets_export"),
  snippetsImport: () => call<SnippetImportReport | null>("snippets_import"),

  // Clipboard Hub (see docs/clipboard-hub.md)
  clipboardList: (filter?: ClipFilter) => call<{ items: ClipItem[]; total: number }>("clipboard_list", { filter }),
  clipboardImage: (id: number) => call<string>("clipboard_image", { id }),
  clipboardDetails: (id: number) => call<ClipDetails>("clipboard_details", { id }),
  clipboardCopy: (id: number) => call<void>("clipboard_copy", { id }),
  clipboardPaste: (id: number) => call<void>("clipboard_paste", { id }),
  clipboardPin: (id: number, pinned: boolean) => call<void>("clipboard_pin", { id, pinned }),
  clipboardDelete: (id: number) => call<void>("clipboard_delete", { id }),
  clipboardClear: (includePinned = false) => call<number>("clipboard_clear", { includePinned }),
  clipboardStatus: () => call<HubStatus>("clipboard_status"),
  clipboardSetPrivate: (enabled: boolean, minutes?: number) => call<PrivateMode>("clipboard_set_private", { enabled, minutes }),
  quickActionsList: () => call<QuickAction[]>("quick_actions_list"),
  quickActionsSave: (input: QuickActionInput, id?: string) => call<QuickAction[]>("quick_actions_save", { id, input }),
  quickActionsDelete: (id: string) => call<QuickAction[]>("quick_actions_delete", { id }),
  quickActionsReorder: (id: string, delta: number) => call<QuickAction[]>("quick_actions_reorder", { id, delta }),
  quickActionsRun: (id: string, confirm = false) => call<{ output: CommandOutput | null }>("quick_actions_run", { id, confirm }),
  quickActionsPick: (kind: "app" | "file" | "folder") => call<string | null>("quick_actions_pick", { kind }),

  // Desktop Organizer (see docs/desktop-organizer.md)
  organizerPreview: () => call<OrganizerPlan>("organizer_preview"),
  organizerApply: (items: PlanItem[], source: "manual" | "button" = "manual") => call<ApplyReport>("organizer_apply", { items, source }),
  organizerRunNow: () => call<ApplyReport>("organizer_run_now"),
  organizerPending: () => call<{ source: string; created_at: string; files: number } | null>("organizer_pending"),
  organizerDismissPending: () => call<void>("organizer_dismiss_pending"),
  organizerOperations: (limit = 100) => call<OrganizerOperation[]>("organizer_operations", { limit }),
  organizerMoves: (operationId: string) => call<OrganizerMove[]>("organizer_moves", { operationId }),
  organizerUndoLast: () => call<UndoReport>("organizer_undo_last"),
  organizerUndoOperation: (operationId: string) => call<UndoReport>("organizer_undo_operation", { operationId }),
  organizerRestore: (moveIds: number[]) => call<UndoReport>("organizer_restore", { moveIds }),
  organizerRules: () => call<OrganizerRule[]>("organizer_rules"),
  organizerSaveRule: (input: OrganizerRuleInput, id?: string) => call<OrganizerRule[]>("organizer_save_rule", { id, input }),
  organizerDeleteRule: (id: string) => call<OrganizerRule[]>("organizer_delete_rule", { id }),
  organizerReorderRules: (ids: string[]) => call<OrganizerRule[]>("organizer_reorder_rules", { ids }),
  organizerResetRules: () => call<OrganizerRule[]>("organizer_reset_rules"),
  organizerExclude: (name: string) => call<void>("organizer_exclude", { name }),
  organizerPaths: () => call<{ desktop: string; root: string }>("organizer_paths"),
  organizerOpenFolder: (path: string) => call<void>("organizer_open_folder", { path }),

  // Timers and stopwatches
  timersState: () => call<TimersState>("timers_state"),
  timersSave: (input: TimerInput, id?: string) => call<TimersState>("timers_save", { id, input }),
  timersControl: (id: string, action: "start" | "pause" | "reset" | "skip") => call<TimersState>("timers_control", { id, action }),
  timersDelete: (id: string) => call<TimersState>("timers_delete", { id }),
  timerSessions: (limit = 100) => call<TimerSession[]>("timer_sessions", { limit }),
  timerSessionsClear: () => call<void>("timer_sessions_clear"),
  stopwatchesCreate: (name: string) => call<TimersState>("stopwatches_create", { name }),
  stopwatchesRename: (id: string, name: string) => call<TimersState>("stopwatches_rename", { id, name }),
  stopwatchesControl: (id: string, action: "start" | "pause" | "reset" | "lap") => call<TimersState>("stopwatches_control", { id, action }),
  stopwatchesDelete: (id: string) => call<TimersState>("stopwatches_delete", { id }),
  stopwatchesSaveResult: (id: string, note?: string) => call<StopwatchResult[]>("stopwatches_save_result", { id, note }),
  stopwatchResults: (limit = 100) => call<StopwatchResult[]>("stopwatch_results", { limit }),
  stopwatchResultsDelete: (id: number) => call<StopwatchResult[]>("stopwatch_results_delete", { id }),

  automationsList: () => call<Automation[]>("automations_list"),
  automationsCreate: (input: AutomationInput) => call<Automation>("automations_create", { input }),
  automationsUpdate: (id: string, input: AutomationInput) => call<Automation>("automations_update", { id, input }),
  automationsSetEnabled: (id: string, enabled: boolean) => call<Automation>("automations_set_enabled", { id, enabled }),
  automationsDelete: (id: string) => call<void>("automations_delete", { id }),
  automationsReorder: (ids: string[]) => call<Automation[]>("automations_reorder", { ids }),
  automationsRun: (id: string, confirm = false, source: "manual" | "ui_button" = "manual") => call<RunReport>("automations_run", { id, confirm, source }),
  automationsTest: (input: AutomationInput, id?: string, execute = false, confirm = false) => call<RunReport>("automations_test", { id, input, execute, confirm }),
  automationsLogs: (id?: string, limit = 50) => call<AutomationLog[]>("automations_logs", { id, limit }),
  automationsPending: () => call<PendingRun[]>("automations_pending"),
  automationsConfirm: (token: string, approve: boolean) => call<RunReport | null>("automations_confirm", { token, approve }),
  automationsTemplates: () => call<AutomationTemplate[]>("automations_templates"),
  automationsInstallTemplate: (key: string) => call<Automation>("automations_install_template", { key }),
  automationsSchema: () => call<object>("automations_schema"),
  automationsPick: (kind: "app" | "file" | "folder") => call<string | null>("automations_pick", { kind }),
  focusModeGet: () => call<FocusState>("focus_mode_get"),
  focusModeSet: (enabled: boolean, minutes?: number) => call<FocusState>("focus_mode_set", { enabled, minutes }),

  // Developer Tools (see docs/developer-tools.md)
  devPickFolder: (title?: string) => call<string | null>("devtools_pick_folder", { title }),
  gitRepos: () => call<GitRepo[]>("git_repos"),
  gitOpenRepo: (path: string) => call<GitRepo>("git_open_repo", { path }),
  gitForgetRepo: (path: string) => call<void>("git_forget_repo", { path }),
  gitStatus: (repo: string) => call<GitStatus>("git_status", { repo }),
  gitLog: (repo: string, limit?: number) => call<GitCommit[]>("git_log", { repo, limit }),
  gitDiff: (repo: string, opts: { path?: string; staged?: boolean; untracked?: boolean; commit?: string }) => call<GitDiff>("git_diff", { repo, ...opts }),
  gitStage: (repo: string, paths: string[], stage: boolean, confirm = false) => call<GitStatus>("git_stage", { repo, paths, stage, confirm }),
  gitCommit: (repo: string, message: string, confirm = false) => call<GitCommit>("git_commit", { repo, message, confirm }),
  gitPull: (repo: string, confirm = false) => call<GitSync>("git_pull", { repo, confirm }),
  gitPush: (repo: string, confirm = false) => call<GitSync>("git_push", { repo, confirm }),

  projectsList: () => call<Project[]>("projects_list"),
  projectsAdd: (path: string) => call<Project>("projects_add", { path }),
  projectsScan: (dir: string) => call<ProjectCandidate[]>("projects_scan", { dir }),
  projectsUpdate: (id: string, update: { name?: string; kind?: string; commands?: ProjectCommand[]; favorite?: boolean }) => call<Project>("projects_update", { id, update }),
  projectsRedetect: (id: string) => call<Project>("projects_redetect", { id }),
  projectsRemove: (id: string) => call<void>("projects_remove", { id }),
  projectsOpen: (id: string, target: "editor" | "terminal" | "explorer", shell?: ShellId, confirm = false) => call<void>("projects_open", { id, target, shell, confirm }),
  projectsRun: (id: string, index: number, shell?: ShellId, confirm = false) => call<void>("projects_run", { id, index, shell, confirm }),

  terminalShells: () => call<ShellInfo[]>("terminal_shells"),
  terminalHome: () => call<string>("terminal_home"),
  terminalOpen: (shell: ShellId, cwd: string, command?: string, confirm = false) => call<void>("terminal_open", { shell, cwd, command, confirm }),
  terminalRun: (runId: string, shell: ShellId, cwd: string, command: string, confirm = false) => call<TabRun>("terminal_run", { runId, shell, cwd, command, confirm }),
  terminalCancel: (runId: string) => call<boolean>("terminal_cancel", { runId }),
  terminalHistory: (query?: string, limit = 200) => call<TerminalHistoryEntry[]>("terminal_history", { query, limit }),
  terminalHistoryClear: () => call<void>("terminal_history_clear"),
  terminalFavorites: () => call<TerminalFavorite[]>("terminal_favorites"),
  terminalFavoriteSave: (input: { name: string; shell: ShellId; command: string; cwd?: string | null }, id?: string) => call<TerminalFavorite>("terminal_favorite_save", { id, input }),
  terminalFavoriteDelete: (id: string) => call<void>("terminal_favorite_delete", { id }),

  apiSend: (request: ApiRequestSpec, confirm = false) => call<ApiResponse>("api_send", { request, confirm }),
  apiToCurl: (request: ApiRequestSpec, style: "bash" | "cmd" | "powershell") => call<string>("api_to_curl", { request, style }),
  apiWorkspace: () => call<ApiWorkspace>("api_workspace"),
  apiCollectionSave: (name: string, id?: string) => call<string>("api_collection_save", { id, name }),
  apiCollectionDelete: (id: string) => call<void>("api_collection_delete", { id }),
  apiRequestSave: (input: ApiRequestSpec & { name: string; collection_id: string | null }, id?: string) => call<ApiSavedRequest>("api_request_save", { id, input }),
  apiRequestDelete: (id: string) => call<void>("api_request_delete", { id }),
  apiEnvironments: () => call<ApiEnvironment[]>("api_environments"),
  apiEnvironmentSave: (name: string, vars: ApiPair[], id?: string) => call<ApiEnvironment>("api_environment_save", { id, name, vars }),
  apiEnvironmentDelete: (id: string) => call<void>("api_environment_delete", { id }),
  apiEnvironmentActivate: (id: string | null) => call<void>("api_environment_activate", { id }),
  apiHistory: (limit = 100) => call<ApiHistoryEntry[]>("api_history", { limit }),
  apiHistoryClear: () => call<void>("api_history_clear"),

  integrationsList: () => call<IntegrationsOverview>("integrations_list"),
  integrationsSetEnabled: (id: string, enabled: boolean, confirm = false) => call<ConnectorInfo>("integrations_set_enabled", { id, enabled, confirm }),
  integrationsSetOffline: (enabled: boolean) => call<boolean>("integrations_set_offline", { enabled }),
  integrationsUpdateSettings: (id: string, settings: Settings) => call<ConnectorInfo>("integrations_update_settings", { id, settings }),
  integrationsResetSettings: (id: string) => call<ConnectorInfo>("integrations_reset_settings", { id }),
  integrationsSetPermission: (id: string, permission: string, granted: boolean) => call<ConnectorInfo>("integrations_set_permission", { id, permission, granted }),
  integrationsSetSecret: (id: string, secret: string) => call<ConnectorInfo>("integrations_set_secret", { id, secret }),
  integrationsClearSecret: (id: string) => call<ConnectorInfo>("integrations_clear_secret", { id }),
  integrationsCheck: (id?: string) => call<ConnectorInfo[]>("integrations_check", { id }),
  integrationsLogs: (id?: string, limit = 200) => call<ConnectorLog[]>("integrations_logs", { id, limit }),

  regexList: () => call<SavedRegex[]>("regex_list"),
  regexSave: (input: RegexInput, id?: string) => call<SavedRegex>("regex_save", { id, input }),
  regexDelete: (id: string) => call<void>("regex_delete", { id }),

  devjsonProcess: (text: string, op: JsonOp, from?: "json" | "yaml") => call<JsonProcessed>("devjson_process", { text, from, op }),
  devjsonQuery: (text: string, path: string, from?: "json" | "yaml") => call<JsonQueryResult>("devjson_query", { text, from, path }),
};

export const EVENTS = {
  notify: "focusdeck://notify",
  navigate: "focusdeck://navigate",
  modulesChanged: "focusdeck://modules-changed",
  settingsChanged: "focusdeck://settings-changed",
  layoutChanged: "focusdeck://layout-changed",
  clipboardChanged: "focusdeck://clipboard-changed",
  timersChanged: "focusdeck://timers-changed",
  clipboardPrivate: "focusdeck://clipboard-private",
  automationConfirm: "focusdeck://automation-confirm",
  automationRun: "focusdeck://automation-run",
  focusMode: "focusdeck://focus-mode",
  organizerChanged: "focusdeck://organizer-changed",
  actionLog: "focusdeck://action-log",
  terminalOutput: "focusdeck://terminal-output",
} as const;

// --- Developer Tools ---

export interface GitRepo {
  path: string;
  name: string;
  last_opened_at: string;
}

export interface GitFileChange {
  path: string;
  orig_path: string | null;
  /** M A D R C T U ? */
  status: string;
}

export interface GitStatus {
  root: string;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  detached: boolean;
  no_commits: boolean;
  staged: GitFileChange[];
  unstaged: GitFileChange[];
  untracked: GitFileChange[];
  conflicted: GitFileChange[];
  clean: boolean;
}

export interface GitCommit {
  hash: string;
  short: string;
  author: string;
  email: string;
  date: string;
  refs: string;
  subject: string;
}

export interface GitDiff {
  text: string;
  truncated: boolean;
}

export interface GitSync {
  output: string;
  status: GitStatus;
}

export type ShellId = "powershell" | "cmd" | "git_bash" | "wsl" | "custom";

export interface ShellInfo {
  id: ShellId;
  name: string;
  path: string | null;
  available: boolean;
}

export interface ProjectCommand {
  name: string;
  command: string;
  custom: boolean;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  kind: string;
  commands: ProjectCommand[];
  favorite: boolean;
  position: number;
  last_opened_at: string | null;
  exists: boolean;
  is_git: boolean;
}

export interface ProjectCandidate {
  path: string;
  name: string;
  kind: string;
  added: boolean;
}

export interface TabRun {
  run_id: string;
  code: number | null;
  timed_out: boolean;
  cancelled: boolean;
  duration_ms: number;
  cwd: string;
}

export interface TerminalOutput {
  run_id: string;
  stream: "stdout" | "stderr";
  text: string;
}

export interface TerminalHistoryEntry {
  id: number;
  shell: ShellId;
  cwd: string;
  command: string;
  exit_code: number | null;
  duration_ms: number;
  created_at: string;
}

export interface TerminalFavorite {
  id: string;
  name: string;
  shell: ShellId;
  command: string;
  cwd: string | null;
  position: number;
}

export interface ApiPair {
  name: string;
  value: string;
  enabled: boolean;
}

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type BodyType = "none" | "json" | "text" | "form";

export interface ApiRequestSpec {
  method: HttpMethod;
  url: string;
  headers: ApiPair[];
  query: ApiPair[];
  body: string;
  body_type: BodyType;
}

export interface ApiSavedRequest extends ApiRequestSpec {
  id: string;
  collection_id: string | null;
  name: string;
  position: number;
  updated_at: string;
}

export interface ApiWorkspace {
  collections: { id: string; name: string; requests: ApiSavedRequest[] }[];
  loose: ApiSavedRequest[];
}

export interface ApiEnvironment {
  id: string;
  name: string;
  vars: ApiPair[];
  active: boolean;
}

export interface ApiResponse {
  status: number;
  status_text: string;
  headers: [string, string][];
  body: string;
  binary: boolean;
  size: number;
  truncated: boolean;
  duration_ms: number;
  final_url: string;
  missing_vars: string[];
}

export interface ApiHistoryEntry {
  id: number;
  method: HttpMethod;
  url: string;
  status: number | null;
  duration_ms: number;
  size: number;
  error: string | null;
  request: ApiRequestSpec;
  created_at: string;
}

export interface RegexInput {
  name: string;
  pattern: string;
  flags: string;
  sample: string;
  replacement: string;
}

export interface SavedRegex extends RegexInput {
  id: string;
  updated_at: string;
}

export type JsonOp = "format" | "minify" | "validate" | "to_json" | "to_yaml";

export interface JsonError {
  message: string;
  line: number | null;
  column: number | null;
}

export interface JsonProcessed {
  ok: boolean;
  output: string;
  detected: "json" | "yaml";
  error: JsonError | null;
  stats: { objects: number; arrays: number; values: number; depth: number } | null;
}

export interface JsonQueryResult {
  matches: { path: string; value: unknown }[];
  error: JsonError | null;
  in_document: boolean;
}

// --- Integrations ---

export type ConnectorKind = "local" | "cloud";
export type AuthType = "none" | "token" | "api_key" | "oauth2" | "password";
export type ConnectorRisk = "read" | "change" | "dangerous";
export type ConnectorStatus = "ok" | "warning" | "error" | "not_configured" | "offline" | "disabled" | "unknown";

/** Mirrors src-tauri/src/integrations/manifest.rs (format: docs/integration.schema.json). */
export interface ConnectorManifest {
  id: string;
  name: string;
  description: string;
  icon: string;
  version: string;
  kind: ConnectorKind;
  auth: { type: AuthType; label: string; hint: string; optional: boolean };
  permissions: { id: string; name: string; description: string; risk: ConnectorRisk }[];
  actions: { id: string; name: string; description: string; permissions: string[] }[];
  events: { id: string; name: string; description: string }[];
  settings: SettingsSchema;
  health: { kind: "none" | "token" | "executable" | "path" | "local_port"; target?: string | null; setting?: string | null };
  enabled_by_default: boolean;
  docs_url: string;
}

export interface ConnectorInfo {
  manifest: ConnectorManifest;
  enabled: boolean;
  granted: string[];
  has_secret: boolean;
  settings: Settings;
  health_status: ConnectorStatus;
  health_message: string;
  health_checked_at: string | null;
}

export interface IntegrationsOverview {
  offline_mode: boolean;
  connectors: ConnectorInfo[];
}

export interface ConnectorLog {
  id: number;
  connector_id: string;
  level: "info" | "warn" | "error";
  action: string;
  message: string;
  created_at: string;
}

export interface SessionStatus {
  previous_crash: boolean;
  crash_message: string | null;
  restored: boolean;
  restore_pending: boolean;
}
