import { useState } from "react";
import { Link } from "react-router-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowDownCircle, CheckCircle2, ExternalLink, History, RefreshCw, RotateCcw, ShieldAlert } from "lucide-react";
import Markdown from "../components/Markdown";
import { Badge, ErrorBanner, PageHeader, Spinner } from "../components/ui";
import { formatDateTime } from "../i18n";
import { api, errorText } from "../lib/api";
import { useAsync } from "../lib/use-async";
import { useGuarded } from "../lib/use-guarded";
import changelog from "../../CHANGELOG.md?raw";

export default function Updates() {
  const { data, setData, error, setError } = useAsync(api.updatesStatus, []);
  const guarded = useGuarded(setError);
  const [checking, setChecking] = useState(false);
  const [checked, setChecked] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  async function check() {
    setChecking(true);
    try {
      setData(await api.updatesCheck());
      setChecked(true);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setChecking(false);
    }
  }

  return (
    <>
      <PageHeader
        title="Обновления"
        subtitle="Проверка новой версии, список изменений и откат. Сеть используется только с разрешением «Сеть: проверка обновлений»."
        actions={
          <button className="btn-primary" onClick={check} disabled={checking || !data?.configured}>
            <RefreshCw className={`size-4 ${checking ? "animate-spin" : ""}`} /> Проверить обновления
          </button>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {note && (
        <p role="status" className="mb-3 text-emerald-600 dark:text-emerald-400">
          {note}
        </p>
      )}
      {!data && !error && <Spinner />}
      {data && (
        <div className="grid gap-4 lg:grid-cols-2">
          <section className="card flex flex-col gap-3">
            <h2 className="font-semibold">Версия</h2>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
              <dt className="text-zinc-500">Установлена</dt>
              <dd className="select-text">{data.current}</dd>
              <dt className="text-zinc-500">Канал обновлений</dt>
              <dd>{data.configured ? <Badge tone="ok">настроен, пакеты проверяются по подписи</Badge> : <Badge tone="warn">не настроен в этой сборке</Badge>}</dd>
            </dl>
            {!data.configured && (
              <p className="text-xs text-zinc-500">
                Сборка без адреса обновлений и ключа подписи (так собираются dev- и portable-версии). FocusDeck никуда не обращается; новые версии
                устанавливаются вручную. См. «Документация → Релиз».
              </p>
            )}
            {data.available ? (
              <div className="rounded-lg border border-accent-500/40 bg-accent-500/10 p-3">
                <div className="flex items-center gap-2 font-semibold">
                  <ArrowDownCircle className="size-4 text-accent-500" /> Доступна версия {data.available.version}
                  {data.available.date && <span className="text-xs font-normal text-zinc-500">{formatDateTime(data.available.date)}</span>}
                </div>
                {data.available.notes && (
                  <div className="mt-2 max-h-60 overflow-y-auto text-sm">
                    <Markdown source={data.available.notes} />
                  </div>
                )}
                <button className="btn-primary mt-3" onClick={() => guarded.run((c) => api.updatesInstall(c))}>
                  Установить и перезапустить
                </button>
              </div>
            ) : (
              checked && (
                <p className="flex items-center gap-2 text-emerald-600 dark:text-emerald-400">
                  <CheckCircle2 className="size-4" /> Установлена последняя версия
                </p>
              )
            )}
            <p className="text-xs text-zinc-500">
              Автоматическая проверка при запуске включается в{" "}
              <Link className="text-accent-600 underline dark:text-accent-400" to="/control-center?module=updates">
                настройках модуля
              </Link>
              . Установка всегда требует подтверждения.
            </p>
          </section>

          <section className="card flex flex-col gap-3">
            <h2 className="flex items-center gap-2 font-semibold">
              <History className="size-4" /> Откат
            </h2>
            {data.previous ? (
              <>
                <p className="text-zinc-600 dark:text-zinc-300">
                  Перед последним обновлением ({formatDateTime(data.previous.updated_at)}) с версии <b>{data.previous.version}</b> сохранена копия базы:
                </p>
                <code className="text-xs break-all select-text">{data.previous.backup}</code>
                <ol className="list-decimal pl-5 text-zinc-600 dark:text-zinc-300">
                  <li>Подготовьте откат данных — копия заменит базу при следующем запуске.</li>
                  <li>Установите версию {data.previous.version} со страницы релизов поверх текущей.</li>
                </ol>
                <div className="flex flex-wrap gap-2">
                  <button
                    className="btn-secondary"
                    onClick={() =>
                      guarded.run(
                        (c) => api.updatesRollback(c),
                        (p) => {
                          setNote(`Откат данных к версии ${p.version} подготовлен.`);
                          window.dispatchEvent(new Event("focusdeck:session"));
                        },
                      )
                    }
                  >
                    <RotateCcw className="size-4" /> Подготовить откат данных
                  </button>
                  {data.releases_url && (
                    <button className="btn-ghost" onClick={() => openUrl(data.releases_url!)}>
                      Страница релизов <ExternalLink className="size-3.5" />
                    </button>
                  )}
                </div>
              </>
            ) : (
              <p className="flex items-start gap-2 text-zinc-500">
                <ShieldAlert className="mt-0.5 size-4 shrink-0" /> Обновлений через FocusDeck ещё не было — точки отката нет. Ручная резервная копия — в
                «Настройки → Резервная копия».
              </p>
            )}
          </section>

          <section className="card lg:col-span-2">
            <div className="select-text">
              <Markdown source={changelog} />
            </div>
          </section>
        </div>
      )}
      {guarded.dialog}
    </>
  );
}
