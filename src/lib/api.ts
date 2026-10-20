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
  | "status";

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
  module_id: string;
  trigger: Trigger;
  actions: Action[];
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface AutomationInput {
  name: string;
  enabled: boolean;
  /** Owning module; the automation doesn't run while it is off (default "automations"). */
  module_id?: string;
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
  automationsRunNow: (id: string) => call<void>("automations_run_now", { id }),
  automationsLogs: (id?: string, limit = 50) => call<AutomationLog[]>("automations_logs", { id, limit }),
};

export const EVENTS = {
  notify: "focusdeck://notify",
  navigate: "focusdeck://navigate",
  modulesChanged: "focusdeck://modules-changed",
  settingsChanged: "focusdeck://settings-changed",
  layoutChanged: "focusdeck://layout-changed",
  clipboardChanged: "focusdeck://clipboard-changed",
  timersChanged: "focusdeck://timers-changed",
} as const;
