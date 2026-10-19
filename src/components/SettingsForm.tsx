import { FormEvent, useEffect, useState } from "react";
import { RotateCcw } from "lucide-react";
import { Field, Toggle } from "./ui";
import { t } from "../i18n";
import { SchemaProperty, Settings, SettingsSchema } from "../lib/api";

interface Props {
  schema: SettingsSchema;
  value: Settings;
  disabled?: boolean;
  onSave: (next: Settings) => Promise<unknown>;
  onReset: () => void;
}

/** Form generated from a module's settings schema. */
export default function SettingsForm({ schema, value, disabled, onSave, onReset }: Props) {
  const [draft, setDraft] = useState<Settings>(value);
  const [saving, setSaving] = useState(false);
  useEffect(() => setDraft(value), [value]);

  const props = Object.entries(schema.properties ?? {});
  if (props.length === 0) return <p className="text-zinc-500">{t.controlCenter.noSettings}</p>;
  const dirty = JSON.stringify(draft) !== JSON.stringify(value);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    await onSave(draft);
    setSaving(false);
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-3">
      {props.map(([key, p]) => (
        <SettingField key={key} name={key} prop={p} value={draft[key]} disabled={disabled} onChange={(v) => setDraft({ ...draft, [key]: v })} />
      ))}
      <div className="flex flex-wrap gap-2 pt-1">
        <button className="btn-primary" disabled={disabled || !dirty || saving}>
          {t.common.save}
        </button>
        {dirty && (
          <button type="button" className="btn-ghost" onClick={() => setDraft(value)}>
            {t.common.cancel}
          </button>
        )}
        <div className="flex-1" />
        <button type="button" className="btn-secondary" disabled={disabled} onClick={onReset}>
          <RotateCcw className="size-4" />
          {t.controlCenter.reset}
        </button>
      </div>
    </form>
  );
}

function SettingField({
  name,
  prop,
  value,
  disabled,
  onChange,
}: {
  name: string;
  prop: SchemaProperty;
  value: unknown;
  disabled?: boolean;
  onChange: (v: unknown) => void;
}) {
  const label = prop.title ?? name;
  if (prop.type === "boolean") {
    return (
      <div className="flex items-center justify-between gap-4">
        <span>{label}</span>
        <Toggle checked={Boolean(value)} disabled={disabled} onChange={onChange} label={label} />
      </div>
    );
  }
  if (prop.enum) {
    return (
      <Field label={label}>
        <select className="input max-w-xs" value={String(value ?? "")} disabled={disabled} onChange={(e) => onChange(e.target.value)}>
          {prop.enum.map((v) => (
            <option key={String(v)} value={String(v)}>
              {String(v)}
            </option>
          ))}
        </select>
      </Field>
    );
  }
  if (prop.type === "integer" || prop.type === "number") {
    const range = prop.minimum !== undefined && prop.maximum !== undefined ? ` (${prop.minimum}–${prop.maximum})` : "";
    return (
      <Field label={label + range}>
        <input
          className="input max-w-40"
          type="number"
          min={prop.minimum}
          max={prop.maximum}
          step={prop.type === "integer" ? 1 : "any"}
          value={value === undefined || value === null ? "" : String(value)}
          disabled={disabled}
          onChange={(e) => {
            const n = prop.type === "integer" ? parseInt(e.target.value, 10) : parseFloat(e.target.value);
            onChange(Number.isNaN(n) ? undefined : n);
          }}
        />
      </Field>
    );
  }
  return (
    <Field label={label}>
      <input className="input" value={String(value ?? "")} disabled={disabled} onChange={(e) => onChange(e.target.value)} />
    </Field>
  );
}
