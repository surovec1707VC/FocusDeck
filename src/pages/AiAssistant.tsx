import { useCallback, useEffect, useRef, useState } from "react";
import ChatWindow, { ChatItem } from "../components/ai/ChatWindow";
import MessageInput from "../components/ai/MessageInput";
import ModelSelector from "../components/ai/ModelSelector";
import ProfileSelector from "../components/ai/ProfileSelector";
import ProviderSettings from "../components/ai/ProviderSettings";
import SessionHistory from "../components/ai/SessionHistory";
import { AiProfile, AiSession, ProviderKind, aiApi } from "../lib/ai";

const STOP_NOTICES: Record<string, string> = {
  max_tokens: "The reply hit the length limit and was cut off.",
  refusal: "The model declined to answer this request.",
  length: "The reply hit the length limit and was cut off.",
};

export default function AiAssistant() {
  const [profiles, setProfiles] = useState<AiProfile[]>([]);
  const [profileId, setProfileId] = useState("fast_assistant");
  const [provider, setProvider] = useState<ProviderKind>("anthropic");
  const [model, setModel] = useState("");
  const [sessions, setSessions] = useState<AiSession[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [items, setItems] = useState<ChatItem[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [endpointName, setEndpointName] = useState<string | null>(null);
  // Bumped when provider settings change so model lists and defaults reload.
  const [configVersion, setConfigVersion] = useState(0);
  const keyCounter = useRef(0);
  const nextKey = () => `local-${keyCounter.current++}`;

  const refreshSessions = useCallback(() => aiApi.sessions().then(setSessions), []);

  useEffect(() => {
    aiApi.profiles().then(setProfiles);
    refreshSessions();
  }, [refreshSessions]);

  // Provider/model the profile uses now (honours the default provider setting).
  useEffect(() => {
    aiApi.resolveTarget(profileId).then((t) => {
      setProvider(t.provider);
      setModel(t.model);
      setEndpointName(t.endpoint_name);
    });
  }, [profileId, configVersion]);

  function applyProfile(profile: AiProfile) {
    setProfileId(profile.id);
  }

  async function openSession(id: string) {
    setError(null);
    setNotice(null);
    setSessionId(id);
    const messages = await aiApi.messages(id);
    setItems(
      messages
        .filter((m) => m.role === "user" || m.role === "assistant")
        .map((m) => ({ key: m.id, role: m.role as "user" | "assistant", content: m.content })),
    );
    const session = sessions.find((s) => s.id === id);
    const profile = profiles.find((p) => p.id === session?.profile_id);
    if (session && profile) {
      setProfileId(profile.id);
      setProvider(session.provider as ProviderKind);
      setModel(session.model);
    }
  }

  function newChat() {
    setSessionId(null);
    setItems([]);
    setError(null);
    setNotice(null);
  }

  async function deleteSession(id: string) {
    await aiApi.deleteSession(id);
    if (id === sessionId) newChat();
    refreshSessions();
  }

  async function send(text: string) {
    setError(null);
    setNotice(null);
    setStreaming(true);
    const replyKey = nextKey();
    setItems((list) => [
      ...list,
      { key: nextKey(), role: "user", content: text },
      { key: replyKey, role: "assistant", content: "", streaming: true },
    ]);

    const updateReply = (fn: (item: ChatItem) => ChatItem) =>
      setItems((list) => list.map((m) => (m.key === replyKey ? fn(m) : m)));

    try {
      const id = await aiApi.chatStream(
        { session_id: sessionId, profile_id: profileId, provider, model: model || null, message: text },
        (event) => {
          switch (event.type) {
            case "started":
              setSessionId(event.session_id);
              break;
            case "delta":
              updateReply((m) => ({ ...m, content: m.content + event.text }));
              break;
            case "done":
              if (event.stop_reason && STOP_NOTICES[event.stop_reason]) {
                setNotice(STOP_NOTICES[event.stop_reason]);
              }
              break;
            case "error":
              setError(event.message);
              break;
          }
        },
      );
      setSessionId(id);
    } catch (e) {
      setError(String(e));
    } finally {
      // Drop the placeholder bubble if nothing arrived.
      setItems((list) =>
        list
          .map((m) => (m.key === replyKey ? { ...m, streaming: false } : m))
          .filter((m) => m.key !== replyKey || m.content !== ""),
      );
      setStreaming(false);
      refreshSessions();
    }
  }

  return (
    <div className="page ai-assistant">
      <header className="page-header">
        <h1>AI Assistant</h1>
      </header>
      <div className="ai-layout">
        <SessionHistory
          sessions={sessions}
          activeId={sessionId}
          onSelect={openSession}
          onNew={newChat}
          onDelete={deleteSession}
          disabled={streaming}
        />
        <section className="ai-main">
          <div className="ai-toolbar">
            <ProfileSelector
              profiles={profiles}
              value={profileId}
              disabled={streaming}
              onChange={(id) => {
                const p = profiles.find((x) => x.id === id);
                if (p) applyProfile(p);
              }}
            />
            <ModelSelector
              provider={provider}
              model={model}
              endpointName={endpointName}
              refreshKey={configVersion}
              disabled={streaming}
              onProviderChange={(p) => {
                setProvider(p);
                setModel("");
              }}
              onModelChange={setModel}
            />
          </div>
          <ProviderSettings onConfigChanged={() => setConfigVersion((v) => v + 1)} />
          <ChatWindow items={items} error={error} notice={notice} />
          <MessageInput onSend={send} disabled={streaming} />
        </section>
      </div>
    </div>
  );
}
