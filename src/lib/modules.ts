import { invoke } from "@tauri-apps/api/core";

export type ModuleCategory =
  | "core"
  | "ai"
  | "productivity"
  | "automation"
  | "voice"
  | "integrations"
  | "system";

export type ModuleStatus =
  | { state: "running" }
  | { state: "stopped" }
  | { state: "error"; message: string };

export interface SchemaProperty {
  type?: "string" | "boolean" | "integer" | "number" | "array" | "object";
  title?: string;
  description?: string;
  default?: unknown;
  enum?: unknown[];
}

export interface SettingsSchema {
  properties?: Record<string, SchemaProperty>;
}

export type Settings = Record<string, unknown>;

export interface ModuleInfo {
  id: string;
  name: string;
  description: string;
  category: ModuleCategory;
  enabled: boolean;
  requires_restart: boolean;
  permissions: string[];
  settings_schema: SettingsSchema;
  settings: Settings;
  status: ModuleStatus;
}

export interface ModuleLogEntry {
  at: string;
  level: "info" | "warn" | "error";
  message: string;
}

export const CATEGORY_LABELS: Record<ModuleCategory, string> = {
  core: "Core",
  ai: "AI",
  productivity: "Productivity",
  automation: "Automation",
  voice: "Voice",
  integrations: "Integrations",
  system: "System",
};

export const modulesApi = {
  list: () => invoke<ModuleInfo[]>("list_modules"),
  get: (id: string) => invoke<ModuleInfo>("get_module", { id }),
  enable: (id: string) => invoke<ModuleInfo>("enable_module", { id }),
  disable: (id: string) => invoke<ModuleInfo>("disable_module", { id }),
  updateSettings: (id: string, settings: Settings) =>
    invoke<ModuleInfo>("update_module_settings", { id, settings }),
  resetSettings: (id: string) => invoke<ModuleInfo>("reset_module_settings", { id }),
  logs: (id: string) => invoke<ModuleLogEntry[]>("get_module_logs", { id }),
  status: (id: string) => invoke<ModuleStatus>("get_module_status", { id }),
};

export function statusLabel(status: ModuleStatus): string {
  switch (status.state) {
    case "running":
      return "Running";
    case "stopped":
      return "Stopped";
    case "error":
      return `Error: ${status.message}`;
  }
}
