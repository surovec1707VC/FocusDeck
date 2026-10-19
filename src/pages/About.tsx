import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen, ShieldCheck } from "lucide-react";
import { ErrorBanner, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { api } from "../lib/api";
import { useAsync } from "../lib/use-async";

const STACK = ["Tauri 2", "Rust", "SQLite (sqlx)", "tracing", "React 19", "TypeScript", "Vite", "Tailwind CSS 4"];

export default function About() {
  const { data: info, error, setError } = useAsync(api.appInfo);
  const A = t.about;

  const reveal = (path: string) => revealItemInDir(path).catch((e) => setError(String(e)));

  return (
    <>
      <PageHeader title={A.title} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <section className="card">
          <div className="flex items-center gap-3">
            <div className="grid size-12 place-items-center rounded-xl bg-accent-600 text-xl font-bold text-white">F</div>
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
          </dl>
          <h2 className="mt-5 mb-2 font-semibold">{A.stack}</h2>
          <div className="flex flex-wrap gap-1.5">
            {STACK.map((s) => (
              <span key={s} className="rounded-md bg-zinc-200 px-2 py-0.5 text-xs dark:bg-zinc-800">
                {s}
              </span>
            ))}
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
