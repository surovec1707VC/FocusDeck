import { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Small Markdown renderer for notes: headings, paragraphs, lists (incl.
 * task lists), quotes, code blocks, rules, **bold**, *italic*, ~~strike~~,
 * `code` and [links](https://…). Builds React elements — no HTML injection.
 */
export default function Markdown({ source }: { source: string }) {
  return <div className="markdown flex flex-col gap-2 leading-relaxed">{blocks(source)}</div>;
}

function blocks(src: string): ReactNode[] {
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  const out: ReactNode[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const key = out.length;
    if (line.trim().startsWith("```")) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith("```")) code.push(lines[i++]);
      i++;
      out.push(
        <pre key={key} className="overflow-x-auto rounded-md bg-zinc-200/70 p-2 font-mono text-xs dark:bg-zinc-800/70">
          {code.join("\n")}
        </pre>,
      );
      continue;
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(line);
    if (h) {
      const size = ["text-xl", "text-lg", "text-base", "text-sm", "text-sm", "text-sm"][h[1].length - 1];
      out.push(
        <div key={key} role="heading" aria-level={h[1].length} className={`${size} font-semibold`}>
          {inline(h[2])}
        </div>,
      );
      i++;
      continue;
    }
    if (/^\s*([-*_])\s*\1\s*\1[\s\-*_]*$/.test(line)) {
      out.push(<hr key={key} className="border-zinc-300 dark:border-zinc-700" />);
      i++;
      continue;
    }
    if (line.startsWith(">")) {
      const quote: string[] = [];
      while (i < lines.length && lines[i].startsWith(">")) quote.push(lines[i++].replace(/^>\s?/, ""));
      out.push(
        <blockquote key={key} className="border-l-2 border-accent-500/60 pl-3 text-zinc-600 dark:text-zinc-300">
          {blocks(quote.join("\n"))}
        </blockquote>,
      );
      continue;
    }
    // GFM table: header row, separator row, body rows.
    if (line.trim().startsWith("|") && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1] ?? "")) {
      const cells = (l: string) =>
        l
          .trim()
          .replace(/^\|/, "")
          .replace(/\|$/, "")
          .split(/(?<!\\)\|/)
          .map((c) => c.trim().replace(/\\\|/g, "|"));
      const head = cells(line);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i].trim().startsWith("|")) rows.push(cells(lines[i++]));
      out.push(
        <div key={key} className="overflow-x-auto">
          <table className="data-table">
            <thead>
              <tr>
                {head.map((c, j) => (
                  <th key={j}>{inline(c)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((r, ri) => (
                <tr key={ri}>
                  {r.map((c, j) => (
                    <td key={j}>{inline(c)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    const listRe = /^\s*([-*+]|\d+[.)])\s+(.*)$/;
    if (listRe.test(line)) {
      const ordered = /^\s*\d/.test(line);
      const items: ReactNode[] = [];
      while (i < lines.length && listRe.test(lines[i])) {
        const text = listRe.exec(lines[i])![2];
        const task = /^\[([ xX])\]\s+(.*)$/.exec(text);
        items.push(
          <li key={items.length} className={task ? "list-none" : ""}>
            {task ? (
              <span className="inline-flex items-start gap-1.5">
                <input type="checkbox" readOnly checked={task[1] !== " "} className="mt-1 size-3.5 accent-accent-600" tabIndex={-1} />
                <span className={task[1] !== " " ? "text-zinc-400 line-through" : ""}>{inline(task[2])}</span>
              </span>
            ) : (
              inline(text)
            )}
          </li>,
        );
        i++;
      }
      out.push(
        ordered ? (
          <ol key={key} className="list-decimal pl-5">
            {items}
          </ol>
        ) : (
          <ul key={key} className="list-disc pl-5">
            {items}
          </ul>
        ),
      );
      continue;
    }
    if (!line.trim()) {
      i++;
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^(#{1,6}\s|>|```|\s*\||\s*([-*+]|\d+[.)])\s)/.test(lines[i])) para.push(lines[i++]);
    out.push(<p key={key}>{para.flatMap((p, j) => (j ? [<br key={`b${j}`} />, ...inline(p)] : inline(p)))}</p>);
  }
  return out;
}

const INLINE = /(`[^`]+`)|(\*\*[^*]+\*\*|__[^_]+__)|(\*[^*\s][^*]*\*|_[^_\s][^_]*_)|(~~[^~]+~~)|(\[[^\]]+\]\([^)\s]+\))/;

function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  let rest = text;
  while (rest) {
    const m = INLINE.exec(rest);
    if (!m) {
      out.push(rest);
      break;
    }
    if (m.index > 0) out.push(rest.slice(0, m.index));
    const tok = m[0];
    const key = out.length;
    if (m[1]) out.push(<code key={key} className="rounded bg-zinc-200/80 px-1 font-mono text-[0.85em] dark:bg-zinc-800">{tok.slice(1, -1)}</code>);
    else if (m[2]) out.push(<strong key={key}>{inline(tok.slice(2, -2))}</strong>);
    else if (m[3]) out.push(<em key={key}>{inline(tok.slice(1, -1))}</em>);
    else if (m[4]) out.push(<s key={key}>{inline(tok.slice(2, -2))}</s>);
    else {
      const [, label, url] = /^\[([^\]]+)\]\(([^)\s]+)\)$/.exec(tok)!;
      const safe = /^(https?:|mailto:)/i.test(url);
      // A link to another doc (other.md) opens it inside the app.
      const doc = /^([\w-]+)\.md(#.*)?$/.exec(url);
      out.push(
        doc ? (
          <a key={key} href={`#/docs/${doc[1]}`} className="text-accent-600 underline dark:text-accent-400">
            {label}
          </a>
        ) : safe ? (
          <a
            key={key}
            href={url}
            className="text-accent-600 underline dark:text-accent-400"
            onClick={(e) => {
              e.preventDefault();
              openUrl(url).catch(() => undefined);
            }}
          >
            {label}
          </a>
        ) : (
          <span key={key}>{label}</span>
        ),
      );
    }
    rest = rest.slice(m.index + tok.length);
  }
  return out;
}
