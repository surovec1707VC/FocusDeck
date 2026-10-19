import { useCallback, useEffect, useMemo, useState } from "react";
import SkillCard from "../components/skills/SkillCard";
import SkillSettings from "../components/skills/SkillSettings";
import { LoadIssue, SkillInfo, skillsApi } from "../lib/skills";

export default function Skills() {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [issues, setIssues] = useState<LoadIssue[]>([]);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (reload = false) => {
    try {
      setSkills(await (reload ? skillsApi.reload() : skillsApi.list()));
      setIssues(await skillsApi.issues());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function toggle(skill: SkillInfo) {
    try {
      const updated = await skillsApi.setEnabled(skill.manifest.id, !skill.enabled);
      setSkills((list) => list.map((s) => (s.manifest.id === updated.manifest.id ? updated : s)));
    } catch (e) {
      setError(String(e));
    }
  }

  const categories = useMemo(() => Array.from(new Set(skills.map((s) => s.manifest.category))).sort(), [skills]);
  const visible = skills.filter((s) => {
    if (category !== "all" && s.manifest.category !== category) return false;
    const q = query.trim().toLowerCase();
    return !q || s.manifest.name.toLowerCase().includes(q) || s.manifest.description.toLowerCase().includes(q);
  });
  const selected = skills.find((s) => s.manifest.id === selectedId) ?? null;

  return (
    <div className="page">
      <header className="page-header">
        <h1>Skills</h1>
        <div className="filters">
          <input type="search" placeholder="Search skills…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            <option value="all">All categories</option>
            {categories.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
          <button className="secondary" onClick={() => load(true)} title="Re-read skills/*.json">
            Reload
          </button>
        </div>
      </header>

      {error && <p className="error">{error}</p>}
      {issues.length > 0 && (
        <div className="issues">
          <strong>Manifests not loaded</strong>
          {issues.map((i) => (
            <p key={i.file} className="small error">
              {i.file}: {i.errors.join("; ")}
            </p>
          ))}
        </div>
      )}

      <div className="split">
        <ul className="module-list">
          {visible.map((s) => (
            <SkillCard
              key={s.manifest.id}
              skill={s}
              selected={s.manifest.id === selectedId}
              onSelect={() => setSelectedId(s.manifest.id)}
              onToggle={() => toggle(s)}
            />
          ))}
          {visible.length === 0 && <li className="muted">No skills match.</li>}
        </ul>
        {selected ? (
          <SkillSettings key={selected.manifest.id} skill={selected} />
        ) : (
          <div className="details muted">Select a skill to see its prompt, permissions, test run and logs.</div>
        )}
      </div>
    </div>
  );
}
