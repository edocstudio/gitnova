import { useSyncExternalStore } from 'react';
import th from './locales/th.json';
export type Locale = 'en' | 'th';
const STORAGE_KEY = 'gitnova.locale';
const listeners = new Set<() => void>();
export function initialLocale(storage: Pick<Storage, 'getItem'> | null): Locale {
  try { return storage?.getItem(STORAGE_KEY) === 'th' ? 'th' : 'en'; } catch { return 'en'; }
}
function storedLocale(): Locale {
  try { return initialLocale(typeof localStorage === 'undefined' ? null : localStorage); } catch { return 'en'; }
}
let locale: Locale = storedLocale();
export const getLocale = () => locale;
export function setLocale(next: Locale) {
  if (next !== 'en' && next !== 'th') return;
  locale = next;
  try { localStorage.setItem(STORAGE_KEY, next); } catch { /* The current session still uses the selected language. */ }
  if (typeof document !== 'undefined') document.documentElement.lang = next;
  listeners.forEach(listener => listener());
}
export function useLocale() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, getLocale, () => 'en' as Locale);
}
export function translate(language: Locale, message: string, values: readonly unknown[] = []): string {
  const text = language === 'th' ? (th as Record<string, string>)[message] ?? message : message;
  return text.replace(/\{(\d+)\}/g, (original, index: string) => Number(index) < values.length ? String(values[Number(index)]) : original);
}
export function t(message: string, values: readonly unknown[] = []) { return translate(locale, message, values); }
export const dateLocale = () => locale === 'th' ? 'th-TH' : 'en-US';
if (typeof document !== 'undefined') document.documentElement.lang = locale;
if (typeof window !== 'undefined') window.addEventListener('storage', event => {
  if (event.key !== STORAGE_KEY) return;
  locale = event.newValue === 'th' ? 'th' : 'en';
  document.documentElement.lang = locale;
  listeners.forEach(listener => listener());
});
