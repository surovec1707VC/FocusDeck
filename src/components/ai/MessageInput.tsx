import { KeyboardEvent, useState } from "react";

interface Props {
  onSend: (text: string) => void;
  disabled?: boolean;
}

/** Enter sends, Shift+Enter adds a new line. */
export default function MessageInput({ onSend, disabled }: Props) {
  const [text, setText] = useState("");

  function send() {
    const value = text.trim();
    if (!value || disabled) return;
    onSend(value);
    setText("");
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      send();
    }
  }

  return (
    <div className="message-input">
      <textarea
        value={text}
        rows={3}
        placeholder="Ask something… (Enter to send, Shift+Enter for a new line)"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <button onClick={send} disabled={disabled || !text.trim()}>
        Send
      </button>
    </div>
  );
}
