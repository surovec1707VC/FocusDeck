import { useEffect, useState } from "react";
import { Route, Routes, useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import Sidebar from "./components/Sidebar";
import TitleBar from "./components/TitleBar";
import Toasts from "./components/Toasts";
import { EVENTS } from "./lib/api";
import About from "./pages/About";
import Automations from "./pages/Automations";
import ControlCenter from "./pages/ControlCenter";
import ModuleGate from "./components/ModuleGate";
import Dashboard from "./pages/Dashboard";
import Layouts from "./pages/Layouts";
import Modules from "./pages/Modules";
import Permissions from "./pages/Permissions";
import Settings from "./pages/Settings";
import Widgets from "./pages/Widgets";

export default function App() {
  const navigate = useNavigate();
  const [maximized, setMaximized] = useState(false);

  // The tray menu can open a page (e.g. «Настройки»).
  useEffect(() => {
    const unlisten = listen<string>(EVENTS.navigate, (e) => navigate(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [navigate]);

  return (
    // The window is transparent: this shell is the visible surface (rounded unless maximized).
    <div
      className={`flex h-full flex-col overflow-hidden border border-zinc-300/70 bg-zinc-100/95 dark:border-zinc-800 dark:bg-zinc-950/92 ${
        maximized ? "" : "rounded-xl"
      }`}
    >
      <TitleBar onMaximizedChange={setMaximized} />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-y-auto p-4 sm:p-6">
          <div className="mx-auto max-w-6xl">
            <Routes>
              <Route path="/" element={<ModuleGate id="dashboard"><Dashboard /></ModuleGate>} />
              <Route path="/widgets" element={<ModuleGate id="widgets"><Widgets /></ModuleGate>} />
              <Route path="/layouts" element={<ModuleGate id="layouts"><Layouts /></ModuleGate>} />
              <Route path="/automations" element={<ModuleGate id="automations"><Automations /></ModuleGate>} />
              {/* Always reachable: modules can be switched back on here. */}
              <Route path="/control-center" element={<ControlCenter />} />
              <Route path="/modules" element={<ModuleGate id="module_registry"><Modules /></ModuleGate>} />
              <Route path="/settings" element={<ModuleGate id="settings"><Settings /></ModuleGate>} />
              <Route path="/permissions" element={<ModuleGate id="security"><Permissions /></ModuleGate>} />
              <Route path="/about" element={<About />} />
            </Routes>
          </div>
        </main>
      </div>
      <Toasts />
    </div>
  );
}
