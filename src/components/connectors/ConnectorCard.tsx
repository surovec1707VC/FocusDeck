import { AUTH_LABELS, ConnectorInfo, connectorStatusLabel } from "../../lib/connectors";

interface Props {
  connector: ConnectorInfo;
  selected: boolean;
  onSelect: () => void;
  onToggle: () => void;
}

export default function ConnectorCard({ connector, selected, onSelect, onToggle }: Props) {
  const m = connector.manifest;
  return (
    <li className={selected ? "module-card selected" : "module-card"} onClick={onSelect}>
      <div className="module-card-head">
        <div>
          <strong>{m.name}</strong>
          <span className="badge">{m.category}</span>
          <span className="badge">{AUTH_LABELS[m.auth_type]}</span>
        </div>
        <label className="switch" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={connector.enabled}
            onChange={onToggle}
            aria-label={`${connector.enabled ? "Disable" : "Enable"} ${m.name}`}
          />
          <span />
        </label>
      </div>
      <p className="muted">{m.description}</p>
      <span className={`status status-${connector.status.state === "ok" ? "running" : connector.status.state === "error" ? "error" : "stopped"}`}>
        {connectorStatusLabel(connector)}
      </span>
    </li>
  );
}
