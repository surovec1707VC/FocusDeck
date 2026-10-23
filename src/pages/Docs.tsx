import { Link, NavLink, useParams } from "react-router-dom";
import { BookOpen } from "lucide-react";
import Markdown from "../components/Markdown";
import { EmptyState, PageHeader } from "../components/ui";

/** The project docs (docs/*.md), bundled at build time: they work offline. */
const FILES = import.meta.glob("../../docs/*.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

export const DOCS = Object.entries(FILES)
  .map(([path, source]) => ({
    name: path.split("/").pop()!.replace(/\.md$/, ""),
    title: source.match(/^#\s+(.+)$/m)?.[1] ?? path,
    source,
  }))
  .filter((d) => !["setup-report", "environment-check"].includes(d.name))
  .sort((a, b) => (a.name === "architecture" ? -1 : b.name === "architecture" ? 1 : a.title.localeCompare(b.title, "ru")));

export default function Docs() {
  const { name = "architecture" } = useParams();
  const doc = DOCS.find((d) => d.name === name);
  return (
    <>
      <PageHeader title="Документация" subtitle="Встроенная документация FocusDeck — доступна без сети." />
      <div className="grid gap-4 lg:grid-cols-[14rem_minmax(0,1fr)]">
        <nav aria-label="Разделы документации" className="flex flex-row flex-wrap gap-1 lg:flex-col">
          {DOCS.map((d) => (
            <NavLink
              key={d.name}
              to={`/docs/${d.name}`}
              className={({ isActive }) =>
                `rounded-lg px-2.5 py-1.5 transition ${isActive || (d.name === "architecture" && !doc) ? "bg-accent-500/15 text-accent-600 dark:text-accent-400" : "text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-400 dark:hover:bg-zinc-800"}`
              }
            >
              {d.title}
            </NavLink>
          ))}
        </nav>
        <article className="card min-w-0 select-text">
          {doc ? (
            <Markdown source={doc.source} />
          ) : (
            <EmptyState icon={BookOpen} title="Раздел не найден" action={<Link className="btn-secondary" to="/docs">К оглавлению</Link>} />
          )}
        </article>
      </div>
    </>
  );
}
