import { AiProfile } from "../../lib/ai";

interface Props {
  profiles: AiProfile[];
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
}

export default function ProfileSelector({ profiles, value, onChange, disabled }: Props) {
  const current = profiles.find((p) => p.id === value);
  return (
    <label className="field">
      Profile
      <select value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}>
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      {current && <span className="muted small">{current.description}</span>}
    </label>
  );
}
