import { useEffect, useState } from "react";
import { ModelInfo, PROVIDER_LABELS, ProviderKind, aiApi } from "../../lib/ai";

interface Props {
  provider: ProviderKind;
  model: string;
  onProviderChange: (provider: ProviderKind) => void;
  onModelChange: (model: string) => void;
  disabled?: boolean;
  /** Active OpenAI-compatible endpoint, shown next to that provider. */
  endpointName?: string | null;
  /** Change to reload the model list (e.g. after switching endpoint). */
  refreshKey?: number;
}

export default function ModelSelector({
  provider,
  model,
  onProviderChange,
  onModelChange,
  disabled,
  endpointName,
  refreshKey,
}: Props) {
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
  }, [provider, refreshKey]);

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
              {p === "openai_compatible" && endpointName ? `${PROVIDER_LABELS[p]}: ${endpointName}` : PROVIDER_LABELS[p]}
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
