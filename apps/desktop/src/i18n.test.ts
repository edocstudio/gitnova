/// <reference types="vite/client" />
import { describe, expect, it } from 'vitest';
import en from './locales/en.json';
import th from './locales/th.json';
import { initialLocale, translate } from './i18n';
describe('English and Thai localization', () => {
  it('has matching complete catalogs and interpolation contracts', () => {
    expect(Object.keys(th).sort()).toEqual(Object.keys(en).sort());
    for (const [key, value] of Object.entries(th)) {
      expect(value.trim(), key).not.toBe('');
      expect((value.match(/\{\d+\}/g) ?? []).sort(), key).toEqual((key.match(/\{\d+\}/g) ?? []).sort());
    }
    const sources = import.meta.glob<string>('./*.tsx', { query: '?raw', import: 'default', eager: true });
    for (const [file, source] of Object.entries(sources)) {
      for (const match of source.matchAll(/\bt\(("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')/g)) {
        const key = match[1].startsWith('"') ? JSON.parse(match[1]) : match[1].slice(1, -1);
        expect(Object.hasOwn(en, key), `${file}: ${key}`).toBe(true);
      }
    }
  });
  it('uses a safe English default and accepts the persisted Thai preference', () => {
    expect(initialLocale(null)).toBe('en');
    expect(initialLocale({ getItem: () => 'th' })).toBe('th');
    expect(initialLocale({ getItem: () => 'unsupported' })).toBe('en');
    expect(initialLocale({ getItem: () => { throw new Error('blocked storage'); } })).toBe('en');
  });
  it('interpolates repository values exactly once and retains technical diagnostics', () => {
    expect(translate('th', 'Create & open {0}', ['feature/{0}<script>'])).toBe('สร้างและเปิด feature/{0}<script>');
    expect(translate('en', '{0} hour ago', [1])).toBe('1 hour ago');
    expect(translate('th', '{0} hours ago', [2])).toBe('2 ชั่วโมงที่แล้ว');
    expect(translate('th', 'fatal: custom Git diagnostic')).toBe('fatal: custom Git diagnostic');
  });
});
