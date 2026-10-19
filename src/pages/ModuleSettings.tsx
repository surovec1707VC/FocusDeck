import { useEffect, useState } from "react";
import { ModuleInfo, SchemaProperty, Settings, modulesApi } from "../lib/modules";

interface Props {
  module: ModuleInfo;
  onChange: (module: ModuleInfo) => void;
  /** Override persistence (used by connectors); defaults to the module commands. */
  save?: (settings: Settings) => Promise<unknown>;
  reset?: () => Promise<unknown>;
}

/** Settings form generated from a settings_schema. */
export default function ModuleSettings({ module, onChange, save, reset }: Props) {
  const doSave = save ?? ((s: Settings) => modulesApi.updateSettings(module.id, s).then(onChange));
  const doReset = reset ?? (() => modulesApi.resetSettings(module.id).then(onChange));
  const [draft, setDraft] = useState<Settings>(module.settings);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(module.settings);
    setError(null);
  }, [module.id, module.settings]);

  const properties = Object.entries(module.settings_schema.properties ?? {});
  const dirty = JSON.stringify(draft) !== JSON.stringify(module.settings);

  async function run(action: () => Promise<unknown>) {
    setSaving(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (properties.length === 0) {
    return <p className="muted">This module has no settings.</p>;
  }

  return (
    <form
      className="settings-form"
      onSubmit={(e) => {
        e.preventDefault();
        run(() => doSave(draft));
      }}
    >
      {properties.map(([key, prop]) => (
        <Field
          key={key}
          name={key}
          prop={prop}
          value={draft[key]}
          onChange={(value) => setDraft({ ...draft, [key]: value })}
        />
      ))}
      {module.requires_restart && (
        <p className="muted">Changes apply after restarting FocusDeck.</p>
      )}
      {error && <p className="error">{error}</p>}
      <div className="row-actions">
        <button type="submit" disabled={!dirty || saving}>
          Save
        </button>
        <button
          type="button"
          className="secondary"
          disabled={saving}
          onClick={() => run(doReset)}
        >
          Reset to defaults
        </button>
      </div>
    </form>
  );
}

function Field({
  name,
  prop,
  value,
  onChange,
}: {
  name: string;
  prop: SchemaProperty;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const label = prop.title ?? name;
  const id = `setting-${name}`;

  if (prop.enum) {
    return (
      <label htmlFor={id}>
        {label}
        <select
          id={id}
          value={String(value ?? "")}
          onChange={(e) => onChange(prop.enum!.find((v) => String(v) === e.target.value))}
        >
          {prop.enum.map((v) => (
            <option key={String(v)} value={String(v)}>
              {String(v)}
            </option>
          ))}
        </select>
      </label>
    );
  }

  switch (prop.type) {
    case "array":
      return (
        <label htmlFor={id}>
          {label} <span className="muted small">(one per line)</span>
          <textarea
            id={id}
            rows={3}
            value={Array.isArray(value) ? value.join("\n") : ""}
            onChange={(e) => onChange(e.target.value.split("\n").filter((line) => line.trim() !== ""))}
          />
        </label>
      );
    case "boolean":
      return (
        <label htmlFor={id} className="checkbox">
          <input
            id={id}
            type="checkbox"
            checked={Boolean(value)}
            onChange={(e) => onChange(e.target.checked)}
          />
          {label}
        </label>
      );
    case "integer":
    case "number":
      return (
        <label htmlFor={id}>
          {label}
          <input
            id={id}
            type="number"
            step={prop.type === "integer" ? 1 : "any"}
            value={value === undefined ? "" : String(value)}
            onChange={(e) => {
              const n = prop.type === "integer" ? parseInt(e.target.value, 10) : parseFloat(e.target.value);
              onChange(Number.isNaN(n) ? undefined : n);
            }}
          />
        </label>
      );
    default:
      return (
        <label htmlFor={id}>
          {label}
          <input
            id={id}
            type="text"
            value={value === undefined ? "" : String(value)}
            onChange={(e) => onChange(e.target.value)}
          />
        </label>
      );
  }
}
