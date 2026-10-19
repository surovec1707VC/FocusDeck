import { invoke } from "@tauri-apps/api/core";

export type SttProvider = "whisper_cpp" | "faster_whisper" | "windows_speech";
export type TtsProvider = "piper" | "windows_tts";

export const STT_LABELS: Record<SttProvider, string> = {
  whisper_cpp: "whisper.cpp (local)",
  faster_whisper: "faster-whisper (local server)",
  windows_speech: "Windows Speech",
};

export const TTS_LABELS: Record<TtsProvider, string> = {
  piper: "Piper (local)",
  windows_tts: "Windows TTS",
};

export const SAMPLE_RATES = [8000, 16000, 22050, 32000, 44100, 48000];
export const PTT_EVENT = "voice://push-to-talk";

export interface VoiceSettings {
  voice_enabled: boolean;
  microphone: string;
  sample_rate: number;
  language: string;
  wake_word: string;
  push_to_talk: string;
  stt_provider: SttProvider;
  tts_provider: TtsProvider;
  auto_send_to_ai: boolean;
  whisper_cpp_binary: string;
  whisper_cpp_model: string;
  faster_whisper_url: string;
  piper_binary: string;
  piper_voice: string;
}

export interface ProviderStatus {
  id: string;
  name: string;
  kind: "stt" | "tts";
  ready: boolean;
  detail: string;
}

export type Intent =
  | { intent: "open"; target: string }
  | { intent: "create_note"; text: string }
  | { intent: "create_task"; text: string }
  | { intent: "ask_ai"; text: string }
  | { intent: "unknown"; text: string };

export const voiceApi = {
  getSettings: () => invoke<VoiceSettings>("voice_get_settings"),
  updateSettings: (settings: VoiceSettings) => invoke<VoiceSettings>("voice_update_settings", { settings }),
  resetSettings: () => invoke<VoiceSettings>("voice_reset_settings"),
  providerStatus: () => invoke<ProviderStatus[]>("voice_provider_status"),
  routeIntent: (transcript: string) => invoke<Intent>("voice_route_intent", { transcript }),
};
