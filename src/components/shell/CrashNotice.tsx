import { useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { AlertTriangle, CheckCircle2, FolderOpen, RotateCw, ShieldCheck } from "lucide-react";
import { t } from "../../i18n";
import { api, SessionStatus } from "../../lib/api";

const C = t.shell.crash;

/** Shown after an unclean exit, a restore, or while a restore waits for restart. */
export default function CrashNotice() {
  const [s, setS] = useState<SessionStatus | null>(null);
  const [safe, setSafe] = useState(false);
  const refresh = () => api.sessionStatus().then(setS).catch(() => undefined);
  useEffect(() => {
    refresh();
    window.addEventListener("focusdeck:session", refresh);
    return () => window.removeEventListener("focusdeck:session", refresh);
  }, []);
  if (!s || !(s.previous_crash || s.restored || s.restore_pending)) return null;

  const dismiss = async () => {
    await api.sessionAck().catch(() => undefined);
    refresh();
  };

  if (s.restore_pending) {
    return (
      <Banner tone="info" icon={RotateCw} text={C.restorePending}>
        <button className="btn-primary btn-sm" onClick={() => api.appRestart()}>
          {C.restart}
        </button>
      </Banner>
    );
  }
  if (s.restored && !s.previous_crash) {
    return (
      <Banner tone="ok" icon={CheckCircle2} text={C.restored}>
        <button className="btn-secondary btn-sm" onClick={dismiss}>
          {C.dismiss}
        </button>
      </Banner>
    );
  }
  return (
    <Banner tone="warn" icon={AlertTriangle} title={C.title} text={C.text}>
      {s.crash_message && (
        <details className="w-full text-xs">
          <summary className="cursor-pointer">{C.details}</summary>
          <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap select-text">{s.crash_message}</pre>
        </details>
      )}
      <button
        className="btn-secondary btn-sm"
        onClick={async () => {
          const info = await api.appInfo();
          revealItemInDir(info.log_dir).catch(() => undefined);
        }}
      >
        <FolderOpen className="size-3.5" /> {C.logs}
      </button>
      <button
        className="btn-secondary btn-sm"
        disabled={safe}
        onClick={async () => {
          const mode = await api.safeModeGet();
          await api.safeModeSet({ ...mode, enabled: true });
          setSafe(true);
        }}
      >
        <ShieldCheck className="size-3.5" /> {safe ? C.safeModeOn : C.safeMode}
      </button>
      <button className="btn-primary btn-sm" onClick={dismiss} autoFocus>
        {C.dismiss}
      </button>
    </Banner>
  );
}

function Banner({ tone, icon: Icon, title, text, children }: { tone: "warn" | "ok" | "info"; icon: typeof AlertTriangle; title?: string; text: string; children: React.ReactNode }) {
  const tones = {
    warn: "border-amber-500/40 bg-amber-500/10 text-amber-900 dark:text-amber-100",
    ok: "border-emerald-500/40 bg-emerald-500/10 text-emerald-900 dark:text-emerald-100",
    info: "border-accent-500/40 bg-accent-500/10 text-accent-700 dark:text-accent-100",
  };
  return (
    <div role="status" className={`mb-4 flex gap-3 rounded-xl border px-4 py-3 ${tones[tone]}`}>
      <Icon className="mt-0.5 size-5 shrink-0" aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        {title && <div className="font-semibold">{title}</div>}
        <div>{text}</div>
        <div className="flex flex-wrap items-center gap-2">{children}</div>
      </div>
    </div>
  );
}
