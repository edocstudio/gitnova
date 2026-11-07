import { afterEach, describe, expect, it, vi } from 'vitest';
import { DateFormatterCache, formatTimestamp } from './date-format';
import { setLocale } from './i18n';

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); setLocale('en'); });

describe('shared date formatters', () => {
  // This is ICU output conformance, not a timing benchmark. Cold Windows CI
  // needed 16 seconds for the native baselines even with one test worker.
  // Keep the larger deadline local; formatter reuse is asserted separately.
  it('preserves native EN/TH date, time and calendar output', () => {
    const cache = new DateFormatterCache();
    for (const locale of ['en-US', 'th-TH'] as const) {
      for (const seconds of [0, 1700000000, 1793401200]) {
        const date = new Date(seconds * 1000);
        expect(cache.format(seconds, 'calendar', locale)).toBe(date.toLocaleDateString(locale));
        expect(cache.format(seconds, 'shortDate', locale)).toBe(date.toLocaleDateString(locale, {month: 'short', day: 'numeric'}));
        expect(cache.format(seconds, 'dateTime', locale)).toBe(date.toLocaleString(locale));
        expect(cache.format(seconds, 'time', locale)).toBe(date.toLocaleTimeString(locale));
        expect(cache.format(seconds, 'commit', locale)).toBe(date.toLocaleString(locale, {dateStyle: 'medium', timeStyle: 'short'}));
      }
    }
  }, 30_000);

  it('reuses formatters across different timestamps and language switches', () => {
    const cache = new DateFormatterCache();
    const NativeFormatter = Intl.DateTimeFormat;
    let constructions = 0;
    vi.stubGlobal('Intl', {DateTimeFormat: class extends NativeFormatter {
      constructor(...args: ConstructorParameters<typeof NativeFormatter>) { super(...args); constructions++; }
    }});
    for (let index = 0; index < 1000; index++) {
      cache.format(1700000000 + index, 'shortDate', 'en-US');
      cache.format(1700000000 + index, 'shortDate', 'th-TH');
    }
    expect(constructions).toBe(2);
    expect(cache.format(Number.NaN, 'dateTime', 'en-US')).toBe('Invalid Date');
    expect(constructions).toBe(2);
  });

  it('replaces formatters when the system time zone signature changes', () => {
    const cache = new DateFormatterCache();
    const NativeFormatter = Intl.DateTimeFormat;
    let constructions = 0;
    vi.stubGlobal('Intl', {DateTimeFormat: class extends NativeFormatter {
      constructor(...args: ConstructorParameters<typeof NativeFormatter>) { super(...args); constructions++; }
    }});
    const zone = vi.spyOn(Date.prototype, 'toTimeString').mockReturnValue('12:00:00 GMT+0000 (UTC)');
    cache.format(1700000000, 'dateTime', 'en-US');
    cache.format(1700000001, 'dateTime', 'en-US');
    expect(constructions).toBe(1);
    zone.mockReturnValue('12:00:00 GMT+0700 (Indochina Time)');
    cache.format(1700000000, 'dateTime', 'en-US');
    expect(constructions).toBe(2);
  });

  it('uses the current application language without retaining formatted output', () => {
    setLocale('en');
    expect(formatTimestamp(1700000000, 'calendar')).toBe(new Date(1700000000000).toLocaleDateString('en-US'));
    setLocale('th');
    expect(formatTimestamp(1700000000, 'calendar')).toBe(new Date(1700000000000).toLocaleDateString('th-TH'));
  });
});
