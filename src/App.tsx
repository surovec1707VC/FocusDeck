import { useState } from "react";
import ControlCenter from "./pages/ControlCenter";
import Modules from "./pages/Modules";
import "./App.css";

type Page = "control-center" | "modules";

const NAV: { id: Page; label: string }[] = [
  { id: "control-center", label: "Control Center" },
  { id: "modules", label: "Modules" },
];

function App() {
  const [page, setPage] = useState<Page>("control-center");
  const [openModuleId, setOpenModuleId] = useState<string | null>(null);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">FocusDeck</div>
        <nav>
          {NAV.map((item) => (
            <button
              key={item.id}
              className={item.id === page ? "nav-item active" : "nav-item"}
              onClick={() => setPage(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>
      </aside>
      <main className="content">
        {page === "control-center" && <ControlCenter initialSelectedId={openModuleId} />}
        {page === "modules" && (
          <Modules
            onOpen={(id) => {
              setOpenModuleId(id);
              setPage("control-center");
            }}
          />
        )}
      </main>
    </div>
  );
}

export default App;
