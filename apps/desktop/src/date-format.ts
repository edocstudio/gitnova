import { dateLocale } from './i18n';

export type DateFormat = 'calendar' | 'shortDate' | 'dateTime' | 'commit' | 'time';

const OPTIONS: Record<DateFormat, Intl.DateTimeFormatOptions> = {
  calendar: { year: 'numeric', month: 'numeric', day: 'numeric' },
  shortDate: { month: 'short', day: 'numeric' },
  dateTime: {
    year: 'numeric', month: 'numeric', day: 'numeric',
    hour: 'numeric', minute: 'numeric', second: 'numeric',
  },
  commit: { dateStyle: 'medium', timeStyle: 'short' },
  time: { hour: 'numeric', minute: 'numeric', second: 'numeric' },
};

// Retain formatters, never formatted values/commit timestamps. The finite styles
// and two supported locales bound this map to ten entries. Creating ICU-backed
// formatters per visible row needlessly allocates native WebView memory.
export class DateFormatterCache {
  private readonly formatters = new Map<string, Intl.DateTimeFormat>();
  private zone = '';

  format(seconds: number, style: DateFormat, locale: 'en-US' | 'th-TH'): string {
    const date = new Date(seconds * 1000);
    if (!Number.isFinite(date.getTime())) return 'Invalid Date';
    // Detect system zone/DST changes without constructing another Intl formatter.
    const zone = new Date().toTimeString().slice(9);
    if (this.zone !== zone) {
      this.formatters.clear();
      this.zone = zone;
    }
    const key = `${locale}:${style}`;
    let formatter = this.formatters.get(key);
    if (!formatter) {
      formatter = new Intl.DateTimeFormat(locale, OPTIONS[style]);
      this.formatters.set(key, formatter);
    }
    return formatter.format(date);
  }
}

const dateFormatters = new DateFormatterCache();
export function formatTimestamp(seconds: number, style: DateFormat = 'dateTime'): string {
  return dateFormatters.format(seconds, style, dateLocale());
}
