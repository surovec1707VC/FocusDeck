/// <reference lib="webworker" />
// Regex Tester evaluation off the UI thread: a pattern with catastrophic
// backtracking only blocks this worker, which the page terminates on timeout.

export interface RegexJob {
  id: number;
  pattern: string;
  flags: string;
  text: string;
  replacement: string | null;
  max: number;
}

export interface RegexGroup {
  index: number;
  name: string | null;
  text: string | null;
  start: number | null;
  end: number | null;
}

export interface RegexMatch {
  start: number;
  end: number;
  text: string;
  groups: RegexGroup[];
}

export interface RegexResult {
  id: number;
  error: string | null;
  matches: RegexMatch[];
  truncated: boolean;
  replaced: string | null;
  ms: number;
}

self.onmessage = (e: MessageEvent<RegexJob>) => {
  const { id, pattern, flags, text, replacement, max } = e.data;
  const started = performance.now();
  const out: RegexResult = { id, error: null, matches: [], truncated: false, replaced: null, ms: 0 };
  try {
    // `d` gives group positions; it doesn't change what matches.
    const re = new RegExp(pattern, flags.includes("d") ? flags : flags + "d");
    const names = new Map<number, string>();
    // Group names in order of their opening parenthesis.
    let n = 0;
    for (let i = 0; i < pattern.length; i++) {
      if (pattern[i] === "\\") {
        i++;
        continue;
      }
      if (pattern[i] === "(" && pattern[i + 1] !== "?") n++;
      else if (pattern[i] === "(" && pattern[i + 1] === "?" && pattern[i + 2] === "<" && pattern[i + 3] !== "=" && pattern[i + 3] !== "!") {
        n++;
        names.set(n, pattern.slice(i + 3, pattern.indexOf(">", i)));
      }
    }
    const global = re.global || re.sticky;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      if (out.matches.length >= max) {
        out.truncated = true;
        break;
      }
      const idx = (m as RegExpExecArray & { indices?: ([number, number] | undefined)[] }).indices ?? [];
      out.matches.push({
        start: m.index,
        end: m.index + m[0].length,
        text: m[0],
        groups: m.slice(1).map((g, i) => ({ index: i + 1, name: names.get(i + 1) ?? null, text: g ?? null, start: idx[i + 1]?.[0] ?? null, end: idx[i + 1]?.[1] ?? null })),
      });
      if (!global) break;
      // An empty match would loop forever.
      if (m[0].length === 0) re.lastIndex++;
    }
    if (replacement !== null) out.replaced = text.replace(new RegExp(pattern, flags), replacement);
  } catch (err) {
    out.error = err instanceof Error ? err.message : String(err);
  }
  out.ms = Math.round(performance.now() - started);
  self.postMessage(out);
};

// Loading the worker can take a while (first start); the page starts the
// timeout only after this.
self.postMessage({ ready: true });
