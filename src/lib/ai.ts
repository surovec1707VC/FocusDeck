import { Channel, invoke } from "@tauri-apps/api/core";

export type ProviderKind = "ollama" | "openai_compatible";

export const PROVIDER_LABELS: Record<ProviderKind, string> = {
  ollama: "Ollama (local)",
  openai_compatible: "OpenAI-compatible",
};

export interface AiProfile {
  id: string;
  name: string;
  description: string;
  provider: ProviderKind;
  model: string;
  system_prompt: string;
  max_tokens: number;
  effort: string | null;
}

export interface ModelInfo {
  id: string;
  name: string;
}

export interface OpenAiEndpoint {
  id: string;
  name: string;
  base_url: string;
  default_model: string;
}

export interface RouterConfig {
  ollama_url: string;
  ollama_default_model: string;
  openai_endpoints: OpenAiEndpoint[];
  active_openai_endpoint: string;
  /** null = each profile uses its own provider. */
  default_provider: ProviderKind | null;
}

export interface ResolvedTarget {
  provider: ProviderKind;
  model: string;
  endpoint_name: string | null;
}

/**
 * Known OpenAI-compatible services. Only base URLs are preset: model names and
 * free-tier terms change often, so models are picked from the live list.
 */
export const ENDPOINT_PRESETS: { id: string; name: string; base_url: string; keys_url: string }[] = [
  { id: "gemini", name: "Google Gemini", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", keys_url: "https://aistudio.google.com/apikey" },
  { id: "groq", name: "Groq", base_url: "https://api.groq.com/openai/v1", keys_url: "https://console.groq.com/keys" },
  { id: "openrouter", name: "OpenRouter", base_url: "https://openrouter.ai/api/v1", keys_url: "https://openrouter.ai/settings/keys" },
  { id: "mistral", name: "Mistral", base_url: "https://api.mistral.ai/v1", keys_url: "https://console.mistral.ai/api-keys" },
  { id: "openai", name: "OpenAI", base_url: "https://api.openai.com/v1", keys_url: "https://platform.openai.com/api-keys" },
  { id: "lm-studio", name: "LM Studio (local)", base_url: "http://localhost:1234/v1", keys_url: "" },
];

export interface AiSession {
  id: string;
  title: string;
  profile_id: string;
  provider: string;
  model: string;
  created_at: string;
  updated_at: string;
}

export interface AiMessage {
  id: string;
  session_id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  created_at: string;
}

export type StreamEvent =
  | { type: "started"; session_id: string }
  | { type: "delta"; text: string }
  | { type: "done"; stop_reason: string | null }
  | { type: "error"; message: string };

export const aiApi = {
  profiles: () => invoke<AiProfile[]>("ai_list_profiles"),
  getConfig: () => invoke<RouterConfig>("ai_get_config"),
  setConfig: (config: RouterConfig) => invoke<void>("ai_set_config", { config }),
  // endpointId picks an OpenAI-compatible endpoint; omitted = the active one.
  listModels: (provider: ProviderKind, endpointId?: string) =>
    invoke<ModelInfo[]>("ai_list_models", { provider, endpointId }),
  testConnection: (provider: ProviderKind, endpointId?: string) =>
    invoke<void>("ai_test_connection", { provider, endpointId }),
  setApiKey: (provider: ProviderKind, apiKey: string, endpointId?: string) =>
    invoke<void>("ai_set_api_key", { provider, apiKey, endpointId }),
  hasApiKey: (provider: ProviderKind, endpointId?: string) =>
    invoke<boolean>("ai_has_api_key", { provider, endpointId }),
  deleteApiKey: (provider: ProviderKind, endpointId?: string) =>
    invoke<void>("ai_delete_api_key", { provider, endpointId }),
  resolveTarget: (profileId: string) => invoke<ResolvedTarget>("ai_resolve_target", { profileId }),
  sessions: () => invoke<AiSession[]>("ai_list_sessions"),
  messages: (sessionId: string) => invoke<AiMessage[]>("ai_get_messages", { sessionId }),
  deleteSession: (sessionId: string) => invoke<void>("ai_delete_session", { sessionId }),

  /** Streams a reply; `onEvent` receives started/delta/done/error events. */
  chatStream: (
    request: {
      session_id: string | null;
      profile_id: string;
      provider: ProviderKind | null;
      model: string | null;
      message: string;
    },
    onEvent: (event: StreamEvent) => void,
  ) => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return invoke<string>("ai_chat_stream", { request, onEvent: channel });
  },
};
