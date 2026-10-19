import { useEffect, useState } from "react";
import { ModelInfo, PROVIDER_LABELS, ProviderKind, aiApi } from "../../lib/ai";

interface Props {
  provider: ProviderKind;
  model: string;
  onProviderChange: (provider: ProviderKind) => void;
  onModelChange: (model: string) => void;
  disabled?: boolean;
}

export default function ModelSelector({ provider, model, onProviderChange, onModelChange, disabled }: Props) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setModels([]);
    setError(null);
    aiApi
      .listModels(provider)
      .then((list) => !cancelled && setModels(list))
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [provider]);

  const listId = `models-${provider}`;

  return (
    <div className="model-selector">
      <label className="field">
        Provider
        <select
          value={provider}
          disabled={disabled}
          onChange={(e) => onProviderChange(e.target.value as ProviderKind)}
        >
          {(Object.keys(PROVIDER_LABELS) as ProviderKind[]).map((p) => (
            <option key={p} value={p}>
              {PROVIDER_LABELS[p]}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        Model
        {/* Free text with suggestions: the list can be empty while offline or without a key. */}
        <input
          list={listId}
          value={model}
          disabled={disabled}
          placeholder="model id"
          onChange={(e) => onModelChange(e.target.value)}
        />
        <datalist id={listId}>
          {models.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </datalist>
        {error && <span className="muted small" title={error}>Model list unavailable</span>}
      </label>
    </div>
  );
}
