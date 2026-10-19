import { invoke } from "@tauri-apps/api/core";
import { ModuleLogEntry, SettingsSchema, Settings } from "./modules";

export type AuthType = "none" | "api_key" | "oauth" | "local";

export const AUTH_LABELS: Record<AuthType, string> = {
  none: "No auth",
  api_key: "API key",
  oauth: "OAuth",
  local: "Local",
};

export type HealthCheck =
  | { type: "http"; url: string }
  | { type: "executable"; program: string }
  | { type: "credentials" }
  | { type: "ai_provider"; provider: string }
  | { type: "none" };

export interface ConnectorManifest {
  id: string;
  name: string;
  description: string;
  category: string;
  enabled: boolean;
  auth_type: AuthType;
  secret_ref?: string;
  permissions: string[];
  actions: { id: string; description: string; mutating: boolean }[];
  events: { id: string; description: string }[];
  settings_schema: SettingsSchema;
  health_check: HealthCheck;
  docs_url: string;
}

export type ConnectorStatus =
  | { state: "unknown" }
  | { state: "ok" }
  | { state: "error"; message: string };

export interface ConnectorInfo {
  manifest: ConnectorManifest;
  enabled: boolean;
  settings: Settings;
  has_secret: boolean;
  status: ConnectorStatus;
}

export interface LoadIssue {
  file: string;
  errors: string[];
}

export const connectorsApi = {
  list: () => invoke<ConnectorInfo[]>("connectors_list"),
  issues: () => invoke<LoadIssue[]>("connectors_issues"),
  reload: () => invoke<ConnectorInfo[]>("connectors_reload"),
  setEnabled: (id: string, enabled: boolean) => invoke<ConnectorInfo>("connectors_set_enabled", { id, enabled }),
  updateSettings: (id: string, settings: Settings) =>
    invoke<ConnectorInfo>("connectors_update_settings", { id, settings }),
  resetSettings: (id: string) => invoke<ConnectorInfo>("connectors_reset_settings", { id }),
  setSecret: (id: string, secret: string) => invoke<ConnectorInfo>("connectors_set_secret", { id, secret }),
  deleteSecret: (id: string) => invoke<ConnectorInfo>("connectors_delete_secret", { id }),
  test: (id: string) => invoke<ConnectorInfo>("connectors_test", { id }),
  logs: (id: string) => invoke<ModuleLogEntry[]>("connectors_logs", { id }),
};

export function connectorStatusLabel(c: ConnectorInfo): string {
  switch (c.status.state) {
    case "ok":
      return "Check passed";
    case "error":
      return `Error: ${c.status.message}`;
    default:
      return c.manifest.auth_type === "api_key" || c.manifest.auth_type === "oauth"
        ? c.has_secret
          ? "Not tested"
          : "Not configured"
        : "Not tested";
  }
}

export function healthCheckLabel(h: HealthCheck): string {
  switch (h.type) {
    case "http":
      return `HTTP GET ${h.url}`;
    case "executable":
      return `\`${h.program}\` is on PATH`;
    case "credentials":
      return "Credentials are stored";
    case "ai_provider":
      return `AI provider test (${h.provider})`;
    case "none":
      return "No check";
  }
}
