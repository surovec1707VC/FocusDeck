import { invoke } from "@tauri-apps/api/core";
import { ProviderKind } from "./ai";

export type SafetyLevel = "read_only" | "confirm_before_action" | "dangerous";

export const SAFETY_LABELS: Record<SafetyLevel, string> = {
  read_only: "Read only",
  confirm_before_action: "Confirm before action",
  dangerous: "Dangerous",
};

export interface JsonSchema {
  type?: string;
  required?: string[];
  properties?: Record<string, JsonSchema & { description?: string }>;
  items?: JsonSchema;
  enum?: unknown[];
}

export interface SkillManifest {
  id: string;
  name: string;
  description: string;
  category: string;
  enabled: boolean;
  required_permissions: string[];
  allowed_tools: string[];
  system_prompt: string;
  input_schema: JsonSchema;
  output_schema: JsonSchema;
  safety_level: SafetyLevel;
  max_steps: number;
  model_profile: string;
}

export interface SkillInfo {
  manifest: SkillManifest;
  enabled: boolean;
  warnings: string[];
  source_file: string;
}

export interface LoadIssue {
  file: string;
  errors: string[];
}

export type StepStatus =
  | { status: "proposed"; needs_confirmation: boolean }
  | { status: "blocked"; reason: string };

export type PlannedStep = { tool: string; args: unknown; reason: string } & StepStatus;

export interface SkillRunOutcome {
  summary: string;
  result: unknown;
  plan: PlannedStep[];
  requires_confirmation: boolean;
  warnings: string[];
}

export interface SkillRunLog {
  task_id: string;
  status: string;
  created_at: string;
  entries: { step: number; level: string; message: string; created_at: string }[];
}

export const skillsApi = {
  list: () => invoke<SkillInfo[]>("skills_list"),
  issues: () => invoke<LoadIssue[]>("skills_issues"),
  reload: () => invoke<SkillInfo[]>("skills_reload"),
  setEnabled: (id: string, enabled: boolean) => invoke<SkillInfo>("skills_set_enabled", { id, enabled }),
  testRun: (id: string, input: unknown, provider: ProviderKind | null, model: string | null) =>
    invoke<SkillRunOutcome>("skills_test_run", { request: { id, input, provider, model } }),
  logs: (id: string) => invoke<SkillRunLog[]>("skills_logs", { id }),
};

/** Example input built from the schema, used to prefill the test-run editor. */
export function sampleInput(schema: JsonSchema): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const key of schema.required ?? []) {
    const prop = schema.properties?.[key];
    switch (prop?.type) {
      case "array":
        out[key] = [];
        break;
      case "object":
        out[key] = {};
        break;
      case "integer":
      case "number":
        out[key] = 0;
        break;
      case "boolean":
        out[key] = false;
        break;
      default:
        out[key] = "";
    }
  }
  return out;
}
