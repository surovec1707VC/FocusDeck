// UI smoke test: drives a running FocusDeck (dev build) over the WebView2
// DevTools protocol, opens every page, the command palette and the shortcuts
// help, and fails on a crashed page (ErrorBoundary), an empty page, or a
// console error. Read-only: it changes no data and no settings.
//
// 1. Start the app with a debug port:
//      $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223 --remote-allow-origins=*'
//      npm run tauri dev
// 2. npm run test:smoke            (FOCUSDECK_CDP_PORT=9223 by default)

const PORT = process.env.FOCUSDECK_CDP_PORT ?? "9223";
const ROUTES = [
  "/", "/widgets", "/layouts", "/notes", "/tasks", "/snippets", "/timers", "/clipboard", "/organizer", "/windows",
  "/dev", "/integrations", "/automations", "/control-center", "/modules", "/settings", "/permissions", "/updates",
  "/about", "/docs", "/docs/security", "/design",
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  const pages = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
  const page = pages.find((p) => p.type === "page" && /localhost|tauri/.test(p.url));
  if (!page) throw new Error("FocusDeck page not found on the debug port");
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
  let id = 0;
  const pending = new Map();
  const errors = [];
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      pending.get(m.id)(m);
      pending.delete(m.id);
    } else if (m.method === "Runtime.exceptionThrown") {
      errors.push(m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
    } else if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") {
      errors.push(m.params.args.map((a) => a.value ?? a.description).join(" "));
    }
  };
  const send = (method, params = {}) =>
    new Promise((r) => {
      const n = ++id;
      pending.set(n, r);
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", { expression: `(async () => { ${expression} })()`, awaitPromise: true, returnByValue: true });
    if (r.result.exceptionDetails) throw new Error(r.result.exceptionDetails.text);
    return r.result.result.value;
  };
  await send("Runtime.enable");
  return { ws, evaluate, errors };
}

const { ws, evaluate, errors } = await connect();
const failures = [];
const report = (ok, name, detail = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(name);
};

// Close the first-run tour without marking it done.
await evaluate(`document.querySelectorAll("dialog[open]").forEach((d) => d.close()); return true;`);

for (const route of ROUTES) {
  const before = errors.length;
  const state = await evaluate(`
    location.hash = "#${route}";
    await new Promise((r) => setTimeout(r, 900));
    const main = document.querySelector("main");
    return {
      crashed: !!document.querySelector("main [role=alert] h1"),
      text: (main?.innerText ?? "").trim().length,
      title: document.querySelector("main h1")?.textContent ?? "",
    };
  `);
  const consoleErrors = errors.slice(before).filter((e) => !/ResizeObserver/.test(e));
  const ok = !state.crashed && state.text > 20 && consoleErrors.length === 0;
  report(ok, `page ${route}`, ok ? state.title : JSON.stringify({ ...state, consoleErrors }));
}

const palette = await evaluate(`
  location.hash = "#/";
  await new Promise((r) => setTimeout(r, 500));
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", code: "KeyK", ctrlKey: true, bubbles: true }));
  await new Promise((r) => setTimeout(r, 400));
  const n = document.querySelectorAll("dialog[open] [role=option]").length;
  document.querySelectorAll("dialog[open]").forEach((d) => d.close());
  return n;
`);
report(palette > 5, "command palette (Ctrl+K)", `${palette} items`);

const help = await evaluate(`
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "F1", bubbles: true }));
  await new Promise((r) => setTimeout(r, 400));
  const ok = !!document.querySelector("dialog[open] table");
  document.querySelectorAll("dialog[open]").forEach((d) => d.close());
  return ok;
`);
report(help, "shortcuts help (F1)");

const ipc = await evaluate(`
  const inv = window.__TAURI_INTERNALS__.invoke;
  const m = await inv("list_modules");
  let kind = null;
  try { await inv("get_module", { id: "definitely_missing" }); } catch (e) { kind = e.kind; }
  return { modules: m.length, kind };
`);
report(ipc.modules >= 40 && ipc.kind === "not_found", "IPC round trip", JSON.stringify(ipc));

ws.close();
console.log(failures.length ? `\n${failures.length} failed` : "\nall passed");
process.exit(failures.length ? 1 : 0);
