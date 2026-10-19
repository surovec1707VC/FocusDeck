import { useCallback, useEffect, useState } from "react";
import ModelSelector from "../ai/ModelSelector";
import { AiProfile, ProviderKind, aiApi } from "../../lib/ai";
import {
  SAFETY_LABELS,
  SkillInfo,
  SkillRunLog,
  SkillRunOutcome,
  sampleInput,
  skillsApi,
} from "../../lib/skills";

type Tab = "overview" | "prompt" | "test" | "logs";

export default function SkillSettings({ skill }: { skill: SkillInfo }) {
  const [tab, setTab] = useState<Tab>("overview");
  const m = skill.manifest;

  return (
    <section className="details">
      <header className="details-head">
        <div>
          <h2>{m.name}</h2>
          <p className="muted">{m.description}</p>
        </div>
      </header>
      <p className="small">
        <span className={`safety safety-${m.safety_level}`}>{SAFETY_LABELS[m.safety_level]}</span> · max{" "}
        {m.max_steps} step(s) · profile <code>{m.model_profile}</code> · <code>skills/{skill.source_file}</code>
      </p>
      {skill.warnings.map((w) => (
        <p key={w} className="small log-warn">
          {w}
        </p>
      ))}

      <nav className="tabs">
        {(["overview", "prompt", "test", "logs"] as Tab[]).map((t) => (
          <button key={t} className={t === tab ? "tab active" : "tab"} onClick={() => setTab(t)}>
            {{ overview: "Permissions & tools", prompt: "System prompt", test: "Test run", logs: "Logs" }[t]}
          </button>
        ))}
      </nav>

      {tab === "overview" && (
        <div className="skill-overview">
          <h3>Required permissions</h3>
          {m.required_permissions.length ? (
            <ul className="permissions">
              {m.required_permissions.map((p) => (
                <li key={p}>
                  <code>{p}</code>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">None.</p>
          )}
          <h3>Allowed tools</h3>
          {m.allowed_tools.length ? (
            <ul className="permissions">
              {m.allowed_tools.map((t) => (
                <li key={t}>
                  <code>{t}</code>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">None — the skill answers from its input only.</p>
          )}
          <h3>Input schema</h3>
          <pre className="code">{JSON.stringify(m.input_schema, null, 2)}</pre>
          <h3>Output schema</h3>
          <pre className="code">{JSON.stringify(m.output_schema, null, 2)}</pre>
        </div>
      )}

      {tab === "prompt" && <pre className="code prompt">{m.system_prompt}</pre>}

      {tab === "test" && <TestRun skill={skill} />}

      {tab === "logs" && <Logs id={m.id} />}
    </section>
  );
}

function TestRun({ skill }: { skill: SkillInfo }) {
  const m = skill.manifest;
  const [input, setInput] = useState(() => JSON.stringify(sampleInput(m.input_schema), null, 2));
  const [profile, setProfile] = useState<AiProfile | null>(null);
  const [provider, setProvider] = useState<ProviderKind>("anthropic");
  const [model, setModel] = useState("");
  const [running, setRunning] = useState(false);
  const [outcome, setOutcome] = useState<SkillRunOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    aiApi.profiles().then((list) => {
      const p = list.find((x) => x.id === m.model_profile) ?? null;
      setProfile(p);
      if (p) {
        setProvider(p.provider);
        setModel(p.model);
      }
    });
  }, [m.model_profile]);

  async function run() {
    setError(null);
    setOutcome(null);
    let parsed: unknown;
    try {
      parsed = JSON.parse(input);
    } catch (e) {
      setError(`Input is not valid JSON: ${e}`);
      return;
    }
    setRunning(true);
    try {
      const overrideProvider = profile && provider === profile.provider ? null : provider;
      const overrideModel = profile && model === profile.model ? null : model;
      setOutcome(await skillsApi.testRun(m.id, parsed, overrideProvider, overrideModel));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="test-run">
      <p className="muted small">
        The skill only returns a plan. Nothing in the plan is executed during a test run.
      </p>
      <ModelSelector
        provider={provider}
        model={model}
        disabled={running}
        onProviderChange={(p) => {
          setProvider(p);
          setModel(profile && p === profile.provider ? profile.model : "");
        }}
        onModelChange={setModel}
      />
      <label className="field">
        Input (JSON)
        <textarea className="code" rows={8} value={input} onChange={(e) => setInput(e.target.value)} />
      </label>
      <div className="row-actions">
        <button onClick={run} disabled={running || !skill.enabled}>
          {running ? "Running…" : "Run test"}
        </button>
        {!skill.enabled && <span className="muted small">Enable the skill to run it.</span>}
      </div>
      {error && <p className="error">{error}</p>}
      {outcome && (
        <div className="outcome">
          {outcome.summary && <p>{outcome.summary}</p>}
          {outcome.warnings.map((w) => (
            <p key={w} className="small log-warn">
              {w}
            </p>
          ))}
          <h3>Result</h3>
          <pre className="code">{JSON.stringify(outcome.result, null, 2)}</pre>
          <h3>Plan {outcome.requires_confirmation && <span className="small log-warn">(needs your confirmation)</span>}</h3>
          {outcome.plan.length === 0 ? (
            <p className="muted">No tool calls proposed.</p>
          ) : (
            <ol className="plan">
              {outcome.plan.map((s, i) => (
                <li key={i}>
                  <code>{s.tool}</code>{" "}
                  {s.status === "blocked" ? (
                    <span className="error small">blocked: {s.reason}</span>
                  ) : s.needs_confirmation ? (
                    <span className="log-warn small">needs confirmation</span>
                  ) : (
                    <span className="status-running small">read-only</span>
                  )}
                  {s.status !== "blocked" && s.reason && <div className="muted small">{s.reason}</div>}
                  <pre className="code small">{JSON.stringify(s.args)}</pre>
                </li>
              ))}
            </ol>
          )}
        </div>
      )}
    </div>
  );
}

function Logs({ id }: { id: string }) {
  const [runs, setRuns] = useState<SkillRunLog[]>([]);
  const refresh = useCallback(() => skillsApi.logs(id).then(setRuns), [id]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  if (runs.length === 0) return <p className="muted">No runs yet.</p>;
  return (
    <div className="skill-logs">
      <button className="secondary" onClick={refresh}>
        Refresh
      </button>
      {runs.map((r) => (
        <div key={r.task_id} className="run">
          <div className="small">
            <strong>{new Date(r.created_at + "Z").toLocaleString()}</strong> · {r.status}
          </div>
          <ol className="logs">
            {r.entries.map((e, i) => (
              <li key={i} className={`log-${e.level}`}>
                {e.message}
              </li>
            ))}
          </ol>
        </div>
      ))}
    </div>
  );
}
