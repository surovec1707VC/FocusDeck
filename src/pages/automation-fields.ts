/** Field descriptions for the visual automation editor: one table per
 *  trigger, condition and action type drives the generic form. */
import { Action, Condition, Trigger } from "../lib/api";

export type FieldKind = "text" | "textarea" | "number" | "bool" | "days" | "time" | "select" | "hotkey" | "layout" | "module" | "path" | "url";

export interface Field {
  key: string;
  label: string;
  kind: FieldKind;
  options?: [string, string][];
  placeholder?: string;
  min?: number;
  max?: number;
  /** For `path`: what the file dialog picks. */
  pick?: "app" | "file" | "folder";
}

const CHANGE: [string, string][] = [
  ["any", "любое изменение"],
  ["up", "подключение / появление"],
  ["down", "отключение / пропадание"],
];
const PROFILES: [string, string][] = [
  ["", "—"],
  ["work", "Работа"],
  ["coding", "Программирование"],
  ["streaming", "Стрим"],
  ["gaming", "Игры"],
  ["focus", "Фокус"],
  ["custom", "Свой"],
];
const proc = (label = "Процесс"): Field => ({ key: "process", label, kind: "text", placeholder: "obs64.exe" });

export const TRIGGERS: Record<Trigger["type"], { label: string; fields: Field[]; defaults: Trigger }> = {
  app_start: { label: "Запуск FocusDeck", fields: [], defaults: { type: "app_start" } },
  system_start: {
    label: "Старт системы",
    fields: [{ key: "within_minutes", label: "Не позже чем через (мин) после загрузки Windows", kind: "number", min: 1, max: 120 }],
    defaults: { type: "system_start", within_minutes: 10 },
  },
  process_started: { label: "Запуск приложения", fields: [proc()], defaults: { type: "process_started", process: "" } },
  process_stopped: { label: "Закрытие приложения", fields: [proc()], defaults: { type: "process_stopped", process: "" } },
  window_focus: {
    label: "Активное окно",
    fields: [proc("Программа (необязательно)"), { key: "title_contains", label: "Заголовок содержит (необязательно)", kind: "text" }],
    defaults: { type: "window_focus", process: "", title_contains: "" },
  },
  hotkey: { label: "Горячая клавиша", fields: [{ key: "shortcut", label: "Сочетание", kind: "hotkey" }], defaults: { type: "hotkey", shortcut: "" } },
  time: {
    label: "Время",
    fields: [
      { key: "time", label: "Время", kind: "time" },
      { key: "days", label: "Дни (пусто — каждый день)", kind: "days" },
    ],
    defaults: { type: "time", time: "09:00", days: [1, 2, 3, 4, 5] },
  },
  interval: { label: "Интервал", fields: [{ key: "minutes", label: "Каждые (мин)", kind: "number", min: 1, max: 1440 }], defaults: { type: "interval", minutes: 30 } },
  display_change: { label: "Монитор подключён / отключён", fields: [{ key: "event", label: "Событие", kind: "select", options: CHANGE }], defaults: { type: "display_change", event: "any" } },
  network_change: { label: "Изменение сети", fields: [{ key: "event", label: "Событие", kind: "select", options: CHANGE }], defaults: { type: "network_change", event: "any" } },
  manual: { label: "Ручной запуск", fields: [], defaults: { type: "manual" } },
  ui_button: { label: "Кнопка в интерфейсе", fields: [{ key: "label", label: "Подпись кнопки", kind: "text" }], defaults: { type: "ui_button", label: "" } },
};

export const CONDITIONS: Record<Condition["type"], { label: string; fields: Field[]; defaults: Condition }> = {
  process_running: {
    label: "Процесс запущен",
    fields: [proc(), { key: "running", label: "Запущен", kind: "bool" }],
    defaults: { type: "process_running", process: "", running: true },
  },
  active_app: { label: "Активное приложение", fields: [proc("Программа")], defaults: { type: "active_app", process: "" } },
  weekday: { label: "День недели", fields: [{ key: "days", label: "Дни", kind: "days" }], defaults: { type: "weekday", days: [1, 2, 3, 4, 5] } },
  time_range: {
    label: "Время",
    fields: [
      { key: "from", label: "С", kind: "time" },
      { key: "to", label: "До", kind: "time" },
    ],
    defaults: { type: "time_range", from: "09:00", to: "18:00" },
  },
  module_enabled: {
    label: "Статус модуля",
    fields: [
      { key: "module_id", label: "Модуль", kind: "module" },
      { key: "enabled", label: "Включён", kind: "bool" },
    ],
    defaults: { type: "module_enabled", module_id: "notifications", enabled: true },
  },
  active_layout: { label: "Активный макет", fields: [{ key: "layout_id", label: "Макет", kind: "layout" }], defaults: { type: "active_layout", layout_id: "" } },
  internet: { label: "Наличие интернета", fields: [{ key: "online", label: "Интернет есть", kind: "bool" }], defaults: { type: "internet", online: true } },
};

export const ACTIONS: Record<Action["type"], { label: string; fields: Field[]; defaults: Action; dangerous?: boolean }> = {
  open_app: {
    label: "Открыть приложение",
    fields: [
      { key: "path", label: "Программа (.exe, .lnk)", kind: "path", pick: "app", placeholder: "%LOCALAPPDATA%\\Programs\\…\\app.exe" },
      { key: "args", label: "Аргументы (только для .exe)", kind: "text" },
    ],
    defaults: { type: "open_app", path: "", args: "" },
  },
  close_app: {
    label: "Закрыть приложение",
    fields: [proc(), { key: "force", label: "Принудительно (без сохранения)", kind: "bool" }],
    defaults: { type: "close_app", process: "", force: false },
    dangerous: true,
  },
  open_url: { label: "Открыть URL", fields: [{ key: "url", label: "Адрес", kind: "url", placeholder: "https://…" }], defaults: { type: "open_url", url: "https://" } },
  open_folder: { label: "Открыть папку", fields: [{ key: "path", label: "Папка", kind: "path", pick: "folder" }], defaults: { type: "open_folder", path: "" } },
  open_file: { label: "Открыть файл", fields: [{ key: "path", label: "Файл", kind: "path", pick: "file" }], defaults: { type: "open_file", path: "" } },
  run_command: {
    label: "Команда терминала",
    fields: [{ key: "command", label: "Команда (cmd)", kind: "textarea", placeholder: "ipconfig /flushdns" }],
    defaults: { type: "run_command", command: "" },
    dangerous: true,
  },
  switch_layout: {
    label: "Переключить макет",
    fields: [
      { key: "layout_id", label: "Макет", kind: "layout" },
      { key: "profile", label: "…или первый макет профиля", kind: "select", options: PROFILES },
    ],
    defaults: { type: "switch_layout", layout_id: "", profile: "work" },
  },
  set_module: {
    label: "Включить / выключить модуль",
    fields: [
      { key: "module_id", label: "Модуль", kind: "module" },
      { key: "enabled", label: "Включить", kind: "bool" },
    ],
    defaults: { type: "set_module", module_id: "notifications", enabled: true },
  },
  notify: {
    label: "Показать уведомление",
    fields: [
      { key: "title", label: "Заголовок", kind: "text" },
      { key: "body", label: "Текст", kind: "text" },
    ],
    defaults: { type: "notify", title: "", body: "" },
  },
  copy_text: { label: "Скопировать текст", fields: [{ key: "text", label: "Текст", kind: "textarea" }], defaults: { type: "copy_text", text: "" } },
  paste_text: { label: "Вставить текст", fields: [{ key: "text", label: "Текст", kind: "textarea" }], defaults: { type: "paste_text", text: "" } },
  screenshot: {
    label: "Сделать скриншот",
    fields: [{ key: "folder", label: "Папка (пусто — Изображения\\FocusDeck)", kind: "path", pick: "folder" }],
    defaults: { type: "screenshot", folder: "" },
  },
  start_timer: {
    label: "Запустить таймер",
    fields: [
      { key: "name", label: "Название", kind: "text" },
      { key: "kind", label: "Тип", kind: "select", options: [["countdown", "Таймер"], ["pomodoro", "Pomodoro"]] },
      { key: "minutes", label: "Минут", kind: "number", min: 1, max: 600 },
    ],
    defaults: { type: "start_timer", name: "Таймер", kind: "countdown", minutes: 25 },
  },
  focus_mode: {
    label: "Focus Mode",
    fields: [
      { key: "enabled", label: "Включить", kind: "bool" },
      { key: "minutes", label: "На (мин, 0 — до выключения)", kind: "number", min: 0, max: 1440 },
    ],
    defaults: { type: "focus_mode", enabled: true, minutes: 25 },
  },
  delete_files: {
    label: "Удалить файлы (в Корзину)",
    fields: [
      { key: "folder", label: "Папка", kind: "path", pick: "folder" },
      { key: "pattern", label: "Маска", kind: "text", placeholder: "*.tmp" },
      { key: "older_than_days", label: "Старше (дней, 0 — любые)", kind: "number", min: 0, max: 3650 },
    ],
    defaults: { type: "delete_files", folder: "", pattern: "*.tmp", older_than_days: 7 },
    dangerous: true,
  },
  log: { label: "Записать в журнал", fields: [{ key: "message", label: "Сообщение", kind: "text" }], defaults: { type: "log", message: "" } },
  show_window: { label: "Показать окно", fields: [], defaults: { type: "show_window" } },
  hide_window: { label: "Скрыть окно в трей", fields: [], defaults: { type: "hide_window" } },
};

const SCRIPT = /\.(exe|bat|cmd|ps1|vbs|js|wsf|msi|com|scr|lnk|hta)$/i;

/** Mirrors `Action::danger` in Rust for live hints in the editor. */
export function isDangerous(a: Action): boolean {
  return Boolean(ACTIONS[a.type]?.dangerous) || (a.type === "open_file" && SCRIPT.test(a.path));
}

export const WEEKDAYS = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

export function describeTrigger(t: Trigger): string {
  const base = TRIGGERS[t.type]?.label ?? t.type;
  switch (t.type) {
    case "process_started":
    case "process_stopped":
      return `${base}: ${t.process}`;
    case "window_focus":
      return `${base}: ${[t.process, t.title_contains && `«${t.title_contains}»`].filter(Boolean).join(" ")}`;
    case "hotkey":
      return `${base}: ${t.shortcut}`;
    case "time":
      return `${t.time}${t.days?.length ? ` (${t.days.map((d) => WEEKDAYS[d - 1]).join(", ")})` : " ежедневно"}`;
    case "interval":
      return `каждые ${t.minutes} мин`;
    case "ui_button":
      return `${base}${t.label ? `: «${t.label}»` : ""}`;
    default:
      return base;
  }
}
