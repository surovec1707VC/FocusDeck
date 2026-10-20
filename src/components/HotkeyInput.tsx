import { KeyboardEvent, useState } from "react";
import { t } from "../i18n";

/** "KeyA" → "A", "Digit1" → "1"; other codes (F5, Space, ArrowUp, Numpad1) as is. */
function keyName(code: string): string | null {
  if (/^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/.test(code)) return null;
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

/** Shortcut text in the format the backend parses: "Ctrl+Alt+K". */
export function shortcutFromEvent(e: KeyboardEvent): string | null {
  const key = keyName(e.code);
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  // Function keys may be used alone; everything else needs a modifier.
  if (!key || (mods.length === 0 && !/^F\d{1,2}$/.test(key))) return null;
  return [...mods, key].join("+");
}

interface Props {
  value: string | null;
  onChange: (shortcut: string | null) => void;
  disabled?: boolean;
  label: string;
}

/** Records a key combination instead of free text. */
export default function HotkeyInput({ value, onChange, disabled, label }: Props) {
  const [recording, setRecording] = useState(false);
  const L = t.layouts;

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>) {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setRecording(false);
      return;
    }
    const shortcut = shortcutFromEvent(e);
    if (shortcut) {
      setRecording(false);
      onChange(shortcut);
    }
  }

  return (
    <span className="flex items-center gap-1">
      <button
        type="button"
        className={`input w-44 text-left font-mono text-xs ${recording ? "ring-2 ring-accent-500" : ""}`}
        disabled={disabled}
        onClick={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={onKeyDown}
        aria-label={label}
        title={L.hotkeyHint}
      >
        {recording ? L.hotkeyPress : (value ?? <span className="text-zinc-500">{L.hotkeyNone}</span>)}
      </button>
      {value && !recording && (
        <button type="button" className="btn-ghost px-2 text-xs" disabled={disabled} onClick={() => onChange(null)}>
          {L.hotkeyClear}
        </button>
      )}
    </span>
  );
}
