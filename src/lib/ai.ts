import { Channel, invoke } from "@tauri-apps/api/core";

export type ProviderKind = "ollama" | "openai_compatible" | "anthropic";

export const PROVIDER_LABELS: Record<ProviderKind, string> = {
  ollama: "Ollama (local)",
  openai_compatible: "OpenAI-compatible",
  anthropic: "Anthropic",
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

export interface RouterConfig {
  ollama_url: string;
  openai_base_url: string;
  openai_default_model: string;
}

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
  listModels: (provider: ProviderKind) => invoke<ModelInfo[]>("ai_list_models", { provider }),
  testConnection: (provider: ProviderKind) => invoke<void>("ai_test_connection", { provider }),
  setApiKey: (provider: ProviderKind, apiKey: string) =>
    invoke<void>("ai_set_api_key", { provider, apiKey }),
  hasApiKey: (provider: ProviderKind) => invoke<boolean>("ai_has_api_key", { provider }),
  deleteApiKey: (provider: ProviderKind) => invoke<void>("ai_delete_api_key", { provider }),
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
