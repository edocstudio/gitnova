import {expect, it} from 'vitest';
import {EDIT_SECTION_LIMIT,fitsUtf8Budget,splitEditorText} from './working-file-text';

it('enforces UTF-8 byte boundaries for ASCII, Thai, emoji and unpaired surrogates', () => {
  for (const text of ['', 'yaml: value\n', 'ภาษาไทย', '😀', '\ud800', '\udc00', '\ud800x', 'a😀ก']) {
    const bytes = new TextEncoder().encode(text).length;
    expect(fitsUtf8Budget(text, bytes)).toBe(true);
    if (bytes) expect(fitsUtf8Budget(text, bytes - 1)).toBe(false);
  }
  expect(fitsUtf8Budget('ก'.repeat(200000), 4 * 1024 * 1024)).toBe(true);
  expect(fitsUtf8Budget('x'.repeat(4 * 1024 * 1024 + 1), 4 * 1024 * 1024)).toBe(false);
});

it('sections preserve all bytes, line endings and surrogate pairs with bounded display text',()=>{
  for(const text of ['', 'setting: ภาษาไทย 😀\n'.repeat(20000), 'x'.repeat(32767)+'😀'+'y'.repeat(100000), '\n'.repeat(100000)]){
    const sections=splitEditorText(text);
    expect(sections.join('')).toBe(text);
    expect(sections.every(section=>section.length<=EDIT_SECTION_LIMIT/2)).toBe(true);
    expect(sections.every(section=>!/[\ud800-\udbff]$/.test(section))).toBe(true);
  }
});
