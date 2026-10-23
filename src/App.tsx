import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import Sidebar, { useNavItems } from "./components/Sidebar";
import CommandPalette from "./components/shell/CommandPalette";
import CrashNotice from "./components/shell/CrashNotice";
import ErrorBoundary from "./components/shell/ErrorBoundary";
import Onboarding from "./components/shell/Onboarding";
import ShortcutsHelp from "./components/shell/ShortcutsHelp";
import UpdateAvailable from "./components/shell/UpdateAvailable";
import { ErrorBanner, Spinner } from "./components/ui";
import { ONBOARDING_KEY, Shell, ShellContext, store } from "./lib/shell-context";
import { t } from "./i18n";
const Docs = lazy(() => import("./pages/Docs"));
const DesignSystem = lazy(() => import("./pages/DesignSystem"));
import TitleBar from "./components/TitleBar";
import Toasts from "./components/Toasts";
import AutomationConfirm from "./components/AutomationConfirm";
import { EVENTS } from "./lib/api";
const About = lazy(() => import("./pages/About"));
const Automations = lazy(() => import("./pages/Automations"));
const ClipboardHub = lazy(() => import("./pages/ClipboardHub"));
import ControlCenter from "./pages/ControlCenter";
import ModuleGate from "./components/ModuleGate";
import Dashboard from "./pages/Dashboard";
const DevTools = lazy(() => import("./pages/DevTools"));
const Integrations = lazy(() => import("./pages/Integrations"));
const Layouts = lazy(() => import("./pages/Layouts"));
const Modules = lazy(() => import("./pages/Modules"));
const Notes = lazy(() => import("./pages/Notes"));
const Organizer = lazy(() => import("./pages/Organizer"));
const Snippets = lazy(() => import("./pages/Snippets"));
const Tasks = lazy(() => import("./pages/Tasks"));
const Timers = lazy(() => import("./pages/Timers"));
const Permissions = lazy(() => import("./pages/Permissions"));
const Settings = lazy(() => import("./pages/Settings"));
const Widgets = lazy(() => import("./pages/Widgets"));
const Updates = lazy(() => import("./pages/Updates"));
const WindowsIntegration = lazy(() => import("./pages/WindowsIntegration"));

export default function App() {
  const navigate = useNavigate();
  const location = useLocation();
  const nav = useNavItems();
  const [maximized, setMaximized] = useState(false);
  const [overlay, setOverlay] = useState<"palette" | "shortcuts" | "onboarding" | null>(() => (store.get(ONBOARDING_KEY) ? null : "onboarding"));
  const [shellError, setShellError] = useState<string | null>(null);
  const shell = useMemo<Shell>(
    () => ({ openPalette: () => setOverlay("palette"), openShortcuts: () => setOverlay("shortcuts"), openOnboarding: () => setOverlay("onboarding") }),
    [],
  );

  // App-wide shortcuts. Typing in a field never triggers the single-key ones.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName));
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && (e.key === "k" || e.key === "K" || e.code === "KeyK")) setOverlay((o) => (o === "palette" ? null : "palette"));
      else if (ctrl && (e.key === "," || e.code === "Comma")) navigate("/settings");
      else if (ctrl && !e.shiftKey && !e.altKey && /^Digit[1-9]$/.test(e.code)) {
        const item = nav[Number(e.code.slice(5)) - 1];
        if (!item) return;
        navigate(item.to);
      } else if (e.key === "F1" || (!typing && e.key === "?")) setOverlay("shortcuts");
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, nav]);

  const closeOverlay = () => {
    if (overlay === "onboarding") store.set(ONBOARDING_KEY, new Date().toISOString());
    setOverlay(null);
  };

  // The tray menu can open a page (e.g. «Настройки»).
  useEffect(() => {
    const unlisten = listen<string>(EVENTS.navigate, (e) => navigate(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [navigate]);

  return (
    // The window is transparent: this shell is the visible surface (rounded unless maximized).
    <ShellContext.Provider value={shell}>
    <a href="#main" onClick={(e) => { e.preventDefault(); document.getElementById("main")?.focus(); }} className="sr-only z-50 rounded-lg bg-accent-600 px-3 py-2 text-white focus:not-sr-only focus:fixed focus:top-2 focus:left-2">
      {t.shell.skip}
    </a>
    <div
      className={`flex h-full flex-col overflow-hidden border border-zinc-300/70 bg-zinc-100/95 dark:border-zinc-800 dark:bg-zinc-950/92 ${
        maximized ? "" : "rounded-xl"
      }`}
    >
      <TitleBar onMaximizedChange={setMaximized} />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main id="main" tabIndex={-1} className="min-w-0 flex-1 overflow-y-auto p-4 outline-none sm:p-6">
          <div className="mx-auto max-w-6xl">
            <CrashNotice />
            <ErrorBanner message={shellError} onClose={() => setShellError(null)} />
            <ErrorBoundary resetKey={location.pathname}>
            <Suspense fallback={<PageLoading />}>
            <Routes>
              <Route path="/" element={<ModuleGate id="dashboard"><Dashboard /></ModuleGate>} />
              <Route path="/widgets" element={<ModuleGate id="widgets"><Widgets /></ModuleGate>} />
              <Route path="/layouts" element={<ModuleGate id="layouts"><Layouts /></ModuleGate>} />
              <Route path="/notes" element={<ModuleGate id="notes"><Notes /></ModuleGate>} />
              <Route path="/tasks" element={<ModuleGate id="tasks"><Tasks /></ModuleGate>} />
              <Route path="/snippets" element={<ModuleGate id="snippets"><Snippets /></ModuleGate>} />
              <Route path="/timers" element={<ModuleGate id="timers"><Timers /></ModuleGate>} />
              <Route path="/clipboard" element={<ModuleGate id="clipboard_history"><ClipboardHub /></ModuleGate>} />
              <Route path="/organizer" element={<ModuleGate id="desktop_organizer"><Organizer /></ModuleGate>} />
              <Route path="/windows" element={<ModuleGate id="windows_integration"><WindowsIntegration /></ModuleGate>} />
              <Route path="/dev" element={<DevTools />} />
              <Route path="/dev/:tool" element={<DevTools />} />
              <Route path="/integrations" element={<ModuleGate id="integrations"><Integrations /></ModuleGate>} />
              <Route path="/automations" element={<ModuleGate id="automations"><Automations /></ModuleGate>} />
              {/* Always reachable: modules can be switched back on here. */}
              <Route path="/control-center" element={<ControlCenter />} />
              <Route path="/modules" element={<ModuleGate id="module_registry"><Modules /></ModuleGate>} />
              <Route path="/settings" element={<ModuleGate id="settings"><Settings /></ModuleGate>} />
              <Route path="/permissions" element={<ModuleGate id="security"><Permissions /></ModuleGate>} />
              <Route path="/about" element={<About />} />
              <Route path="/updates" element={<ModuleGate id="updates"><Updates /></ModuleGate>} />
              <Route path="/docs" element={<Docs />} />
              <Route path="/docs/:name" element={<Docs />} />
              <Route path="/design" element={<DesignSystem />} />
            </Routes>
            </Suspense>
            </ErrorBoundary>
          </div>
        </main>
      </div>
      <Toasts />
      <AutomationConfirm />
      <UpdateAvailable />
      <CommandPalette open={overlay === "palette"} onClose={closeOverlay} onError={setShellError} />
      <ShortcutsHelp open={overlay === "shortcuts"} onClose={closeOverlay} />
      <Onboarding open={overlay === "onboarding"} onClose={closeOverlay} />
    </div>
    </ShellContext.Provider>
  );
}

function PageLoading() {
  return (
    <div className="flex justify-center py-16">
      <Spinner />
    </div>
  );
}
