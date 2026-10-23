import { ReactNode, useState } from "react";
import { Bell, Check, Inbox, Info, Pencil, Plus, Settings, Star, Trash2 } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import Modal from "../components/Modal";
import { Badge, EmptyState, ErrorBanner, ErrorState, Field, IconButton, Kbd, PageHeader, Row, Spinner, Tabs, Toggle, Tooltip } from "../components/ui";

/** Living style guide: every building block of the UI in both themes (docs/design-system.md). */
export default function DesignSystem() {
  const [modal, setModal] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [on, setOn] = useState(true);
  const [tab, setTab] = useState<"a" | "b" | "c">("a");

  return (
    <>
      <PageHeader title="Дизайн-система" subtitle="Токены и компоненты FocusDeck. Подробности — docs/design-system.md." />
      <div className="grid gap-4 xl:grid-cols-2">
        <Section title="Цвета">
          <Swatches
            rows={[
              ["Акцент", ["bg-accent-50", "bg-accent-100", "bg-accent-300", "bg-accent-400", "bg-accent-500", "bg-accent-600", "bg-accent-700"]],
              ["Нейтральные", ["bg-zinc-50", "bg-zinc-200", "bg-zinc-400", "bg-zinc-500", "bg-zinc-700", "bg-zinc-900", "bg-zinc-950"]],
              ["Статусы", ["bg-emerald-500", "bg-amber-500", "bg-red-500", "bg-sky-500"]],
            ]}
          />
        </Section>

        <Section title="Типографика">
          <div className="flex flex-col gap-1.5">
            <div className="text-2xl font-semibold tracking-tight">Заголовок страницы · 24/600</div>
            <div className="text-lg font-semibold">Заголовок карточки · 18/600</div>
            <div className="text-base font-semibold">Заголовок диалога · 16/600</div>
            <div className="font-medium">Подзаголовок, подпись поля · 14/500</div>
            <div>Основной текст · 14/400 — Segoe UI Variable</div>
            <div className="text-xs text-zinc-500">Подсказка, метаданные · 12/400</div>
            <code className="font-mono text-xs">Моноширинный · Cascadia Code</code>
          </div>
        </Section>

        <Section title="Кнопки">
          <div className="flex flex-wrap items-center gap-2">
            <button className="btn-primary"><Plus className="size-4" /> Основная</button>
            <button className="btn-secondary">Вторичная</button>
            <button className="btn-ghost">Тихая</button>
            <button className="btn-danger"><Trash2 className="size-4" /> Опасная</button>
            <button className="btn-primary" disabled>Недоступна</button>
            <button className="btn-secondary btn-sm">Маленькая</button>
            <IconButton label="Редактировать" onClick={() => undefined}><Pencil className="size-4" /></IconButton>
            <IconButton label="Настройки" onClick={() => undefined}><Settings className="size-4" /></IconButton>
          </div>
        </Section>

        <Section title="Поля ввода">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Текст"><input className="input" placeholder="Подсказка в поле" /></Field>
            <Field label="Список"><select className="input"><option>Вариант 1</option><option>Вариант 2</option></select></Field>
            <Field label="Недоступно"><input className="input" disabled value="только чтение" readOnly /></Field>
            <Field label="Многострочное"><textarea className="input min-h-16" defaultValue="Текст" /></Field>
          </div>
        </Section>

        <Section title="Переключатели и строки настроек">
          <div className="divide-y divide-zinc-200 dark:divide-zinc-800">
            <Row title="Включено" hint="Toggle — role=switch, Space/Enter переключают">
              <Toggle checked={on} onChange={setOn} label="Включено" />
            </Row>
            <Row title="Недоступно">
              <Toggle checked={false} onChange={() => undefined} label="Недоступно" disabled />
            </Row>
          </div>
        </Section>

        <Section title="Метки, подсказки, клавиши">
          <div className="flex flex-wrap items-center gap-2">
            <Badge>нейтральная</Badge>
            <Badge tone="accent">акцент</Badge>
            <Badge tone="ok">работает</Badge>
            <Badge tone="warn">внимание</Badge>
            <Badge tone="error">ошибка</Badge>
            <Tooltip text="Подсказка появляется при наведении и фокусе">
              <button className="btn-secondary btn-sm"><Info className="size-3.5" /> Наведите</button>
            </Tooltip>
            <span>
              <Kbd>Ctrl</Kbd> + <Kbd>K</Kbd>
            </span>
            <Spinner />
          </div>
        </Section>

        <Section title="Вкладки">
          <Tabs label="Пример" value={tab} onChange={setTab} tabs={[{ key: "a", label: "Обзор" }, { key: "b", label: "Настройки" }, { key: "c", label: "Журнал" }]} />
          <p className="pt-3 text-zinc-500">Вкладка «{tab}». Стрелки ← → переключают вкладки с клавиатуры.</p>
        </Section>

        <Section title="Модальные окна">
          <div className="flex flex-wrap gap-2">
            <button className="btn-secondary" onClick={() => setModal(true)}>Открыть Modal</button>
            <button className="btn-secondary" onClick={() => setConfirm(true)}>Открыть ConfirmDialog</button>
          </div>
        </Section>

        <Section title="Таблица" wide>
          <table className="data-table">
            <thead>
              <tr><th>Модуль</th><th>Статус</th><th>Версия</th></tr>
            </thead>
            <tbody>
              <tr><td>Виджеты</td><td><Badge tone="ok">работает</Badge></td><td>1.0.0</td></tr>
              <tr><td>Интеграции</td><td><Badge tone="warn">не настроена</Badge></td><td>1.0.0</td></tr>
              <tr><td>Журнал</td><td><Badge>выключен</Badge></td><td>1.0.0</td></tr>
            </tbody>
          </table>
        </Section>

        <Section title="Состояния">
          <div className="flex flex-col gap-3">
            <EmptyState icon={Inbox} title="Пусто" hint="Короткое объяснение и следующий шаг." action={<button className="btn-primary btn-sm"><Plus className="size-3.5" /> Создать</button>} />
            <ErrorBanner message="Встроенная ошибка действия (ErrorBanner)" />
          </div>
        </Section>

        <Section title="Ошибка загрузки">
          <ErrorState message="Сообщение от backend на русском" onRetry={() => undefined} />
        </Section>

        <Section title="Иконки (lucide, 16 px, stroke 2)" wide>
          <div className="flex flex-wrap gap-3 text-zinc-600 dark:text-zinc-300">
            {[Bell, Check, Inbox, Info, Pencil, Plus, Settings, Star, Trash2].map((I, i) => (
              <I key={i} className="size-4" aria-hidden />
            ))}
          </div>
        </Section>
      </div>

      <Modal open={modal} title="Пример окна" onClose={() => setModal(false)} footer={<><button className="btn-secondary" onClick={() => setModal(false)}>Отмена</button><button className="btn-primary" onClick={() => setModal(false)}>Сохранить</button></>}>
        Содержимое окна. Esc, клик по фону и крестик закрывают его; фокус не уходит за пределы окна.
      </Modal>
      <ConfirmDialog open={confirm} title="Удалить?" confirmLabel="Удалить" danger onCancel={() => setConfirm(false)} onConfirm={() => setConfirm(false)}>
        Необратимые действия всегда подтверждаются, по умолчанию в фокусе «Отмена».
      </ConfirmDialog>
    </>
  );
}

function Section({ title, children, wide }: { title: string; children: ReactNode; wide?: boolean }) {
  return (
    <section className={`card ${wide ? "xl:col-span-2" : ""}`}>
      <h2 className="mb-3 font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function Swatches({ rows }: { rows: [string, string[]][] }) {
  return (
    <div className="flex flex-col gap-2">
      {rows.map(([name, classes]) => (
        <div key={name} className="flex items-center gap-2">
          <span className="w-28 shrink-0 text-xs text-zinc-500">{name}</span>
          <div className="flex flex-wrap gap-1">
            {classes.map((c) => (
              <Tooltip key={c} text={c.replace("bg-", "")}>
                <span tabIndex={0} className={`block size-8 rounded-md border border-black/5 ${c}`} />
              </Tooltip>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
