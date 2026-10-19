import { SAFETY_LABELS, SkillInfo } from "../../lib/skills";

interface Props {
  skill: SkillInfo;
  selected: boolean;
  onSelect: () => void;
  onToggle: () => void;
}

export default function SkillCard({ skill, selected, onSelect, onToggle }: Props) {
  const m = skill.manifest;
  return (
    <li className={selected ? "module-card selected" : "module-card"} onClick={onSelect}>
      <div className="module-card-head">
        <div>
          <strong>{m.name}</strong>
          <span className="badge">{m.category}</span>
        </div>
        <label className="switch" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={skill.enabled}
            onChange={onToggle}
            aria-label={`${skill.enabled ? "Disable" : "Enable"} ${m.name}`}
          />
          <span />
        </label>
      </div>
      <p className="muted">{m.description}</p>
      <span className={`safety safety-${m.safety_level}`}>{SAFETY_LABELS[m.safety_level]}</span>
      {skill.warnings.length > 0 && <span className="small log-warn"> · {skill.warnings.length} warning(s)</span>}
    </li>
  );
}
