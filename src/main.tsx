import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import DesktopWidgetWindow from "./components/widgets/DesktopWidgetWindow";
import { ModulesProvider } from "./lib/modules-context";
import { SettingsProvider } from "./lib/settings-context";
import { applyTheme, cachedTheme } from "./lib/theme";
import "./index.css";

// Last used theme (dark by default) until settings arrive from the backend.
applyTheme(cachedTheme());

// Desktop widget windows (label `dw-<widget id>`) render only their widget.
const label = getCurrentWindow().label;
const desktopWidget = label.startsWith("dw-") ? label.slice(3) : null;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <HashRouter>
      <SettingsProvider>
        <ModulesProvider>{desktopWidget ? <DesktopWidgetWindow widgetId={desktopWidget} /> : <App />}</ModulesProvider>
      </SettingsProvider>
    </HashRouter>
  </React.StrictMode>,
);
