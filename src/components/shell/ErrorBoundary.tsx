import { Component, ErrorInfo, ReactNode } from "react";
import { AlertTriangle, Home, RefreshCw, RotateCcw } from "lucide-react";
import { t } from "../../i18n";
import { store } from "../../lib/shell-context";

interface Props {
  children: ReactNode;
  /** Changing it (e.g. the route) clears the error. */
  resetKey?: string;
}

const B = t.shell.boundary;

/** Catches render errors of a page so the rest of the app stays usable. */
export default class ErrorBoundary extends Component<Props, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI error", error, info.componentStack);
    store.set("focusdeck.lastUiError", JSON.stringify({ message: error.message, at: new Date().toISOString() }));
  }

  componentDidUpdate(prev: Props) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="card mx-auto mt-8 flex max-w-xl flex-col items-center gap-3 p-8 text-center">
        <AlertTriangle className="size-10 text-amber-500" aria-hidden />
        <h1 className="text-lg font-semibold">{B.title}</h1>
        <p className="text-zinc-600 dark:text-zinc-300">{B.text}</p>
        <div className="flex flex-wrap justify-center gap-2">
          <button className="btn-primary" onClick={() => this.setState({ error: null })} autoFocus>
            <RotateCcw className="size-4" /> {B.retry}
          </button>
          <a className="btn-secondary" href="#/">
            <Home className="size-4" /> {B.home}
          </a>
          <button className="btn-ghost" onClick={() => location.reload()}>
            <RefreshCw className="size-4" /> {B.reload}
          </button>
        </div>
        <details className="w-full text-left text-xs text-zinc-500">
          <summary className="cursor-pointer">{B.details}</summary>
          <pre className="mt-2 max-h-40 overflow-auto rounded bg-zinc-100 p-2 whitespace-pre-wrap select-text dark:bg-zinc-950">{error.stack ?? error.message}</pre>
        </details>
      </div>
    );
  }
}
