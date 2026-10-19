import { ru } from "./ru";

/** Current UI strings. Only Russian is shipped; the setting is ready for more locales. */
export const t = ru;

export const locale = "ru-RU";

export const formatDateTime = (value: string | null | undefined): string => {
  if (!value) return t.common.never;
  // SQLite's datetime('now') is UTC without a zone marker.
  const iso = /[zZ]|[+-]\d\d:?\d\d$/.test(value) ? value : value.replace(" ", "T") + "Z";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(locale);
};
