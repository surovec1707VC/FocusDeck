import { Link } from "react-router-dom";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { BookOpen, FolderOpen, GraduationCap, Keyboard, Palette, Scale, ShieldCheck } from "lucide-react";
import { ErrorBanner, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { api } from "../lib/api";
import { useShell } from "../lib/shell-context";
import { useAsync } from "../lib/use-async";

/** Direct dependencies and their licenses (from package.json / Cargo metadata). */
const COMPONENTS: { name: string; license: string; part: "ui" | "core" }[] = [
  { name: "Tauri 2", license: "MIT / Apache-2.0", part: "core" },
  { name: "Tauri plugins (dialog, opener, autostart, global-shortcut, single-instance, window-state)", license: "MIT / Apache-2.0", part: "core" },
  { name: "SQLite", license: "Public domain", part: "core" },
  { name: "sqlx", license: "MIT / Apache-2.0", part: "core" },
  { name: "tokio", license: "MIT", part: "core" },
  { name: "serde, serde_json, serde_norway", license: "MIT / Apache-2.0", part: "core" },
  { name: "reqwest", license: "MIT / Apache-2.0", part: "core" },
  { name: "tracing, tracing-subscriber, tracing-appender", license: "MIT", part: "core" },
  { name: "chrono, uuid, base64, anyhow, thiserror", license: "MIT / Apache-2.0", part: "core" },
  { name: "windows, windows-sys", license: "MIT / Apache-2.0", part: "core" },
  { name: "React, React DOM", license: "MIT", part: "ui" },
  { name: "React Router", license: "MIT", part: "ui" },
  { name: "Lucide icons", license: "ISC", part: "ui" },
  { name: "Tailwind CSS", license: "MIT", part: "ui" },
  { name: "Vite", license: "MIT", part: "ui" },
  { name: "TypeScript", license: "Apache-2.0", part: "ui" },
];

export default function About() {
  const { data: info, error, setError } = useAsync(api.appInfo);
  const shell = useShell();
  const A = t.about;

  const reveal = (path: string) => revealItemInDir(path).catch((e) => setError(String(e)));

  return (
    <>
      <PageHeader title={A.title} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <section className="card">
          <div className="flex items-center gap-3">
            <div className="grid size-12 place-items-center rounded-xl bg-accent-600 text-xl font-bold text-white" aria-hidden>
              F
            </div>
            <div>
              <div className="text-lg font-semibold">{t.app.name}</div>
              <div className="text-zinc-500">{t.app.tagline}</div>
            </div>
          </div>
          <dl className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
            <dt className="text-zinc-500">{A.version}</dt>
            <dd className="select-text">{info?.version}</dd>
            <dt className="text-zinc-500">{A.tauri}</dt>
            <dd className="select-text">{info?.tauri_version}</dd>
            <dt className="text-zinc-500">{A.mode}</dt>
            <dd>{info ? (info.portable ? A.portable : A.installed) : ""}</dd>
            <dt className="text-zinc-500">{A.license}</dt>
            <dd>{A.licenseText}</dd>
          </dl>
          <div className="mt-4 flex flex-wrap gap-2">
            <Link to="/docs" className="btn-primary">
              <BookOpen className="size-4" /> {A.docs}
            </Link>
            <Link to="/design" className="btn-secondary">
              <Palette className="size-4" /> {A.design}
            </Link>
            <button className="btn-secondary" onClick={shell.openShortcuts}>
              <Keyboard className="size-4" /> {A.shortcuts}
            </button>
            <button className="btn-secondary" onClick={shell.openOnboarding}>
              <GraduationCap className="size-4" /> {A.onboarding}
            </button>
          </div>
        </section>

        <section className="card">
          <h2 className="mb-2 flex items-center gap-2 font-semibold">
            <ShieldCheck className="size-4 text-emerald-500" />
            {A.privacyTitle}
          </h2>
          <ul className="list-disc space-y-1 pl-5 text-zinc-600 dark:text-zinc-300">
            {A.privacy.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>

        <section className="card lg:col-span-2">
          <h2 className="mb-1 flex items-center gap-2 font-semibold">
            <Scale className="size-4" /> {A.oss}
          </h2>
          <p className="mb-3 text-xs text-zinc-500">{A.ossHint}</p>
          <div className="overflow-x-auto">
            <table className="data-table">
              <thead>
                <tr>
                  <th>{A.component}</th>
                  <th>{A.part}</th>
                  <th>{A.license}</th>
                </tr>
              </thead>
              <tbody>
                {COMPONENTS.map((c) => (
                  <tr key={c.name}>
                    <td>{c.name}</td>
                    <td className="text-zinc-500">{c.part === "core" ? "Rust" : "UI"}</td>
                    <td className="whitespace-nowrap">{c.license}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="card lg:col-span-2">
          <h2 className="mb-2 font-semibold">{A.storage}</h2>
          {info &&
            [
              { label: A.database, path: info.db_file },
              { label: A.logs, path: info.log_dir },
            ].map(({ label, path }) => (
              <div key={label} className="flex flex-wrap items-center gap-2 py-1.5">
                <span className="w-28 text-zinc-500">{label}</span>
                <code className="min-w-0 flex-1 truncate text-xs select-text" title={path}>
                  {path}
                </code>
                <button className="btn-ghost" onClick={() => reveal(path)}>
                  <FolderOpen className="size-4" />
                  {A.showInFolder}
                </button>
              </div>
            ))}
        </section>
      </div>
    </>
  );
}
