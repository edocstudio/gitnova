export const EDIT_SECTION_LIMIT = 64 * 1024;

// Keep DOM layout/selection bounded, including files with very long lines.
export function splitEditorText(text: string): string[] {
  const sections: string[] = [];
  let start = 0;
  while (start < text.length) {
    let end = Math.min(start + EDIT_SECTION_LIMIT / 2, text.length);
    if (end < text.length) {
      const newline = text.lastIndexOf('\n', end - 1);
      if (newline >= start) end = newline + 1;
      const previous = text.charCodeAt(end - 1), next = text.charCodeAt(end);
      if (previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) end--;
    }
    sections.push(text.slice(start, end));
    start = end;
  }
  return sections.length ? sections : [''];
}

// Count UTF-8 bytes without allocating a second encoded copy on every keystroke.
export function fitsUtf8Budget(text: string, limit: number): boolean {
  let bytes = 0;
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    if (code < 0x80) bytes++;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && index + 1 < text.length
      && text.charCodeAt(index + 1) >= 0xdc00 && text.charCodeAt(index + 1) <= 0xdfff) {
      bytes += 4;
      index++;
    } else bytes += 3; // Includes replacement encoding for lone surrogates.
    if (bytes > limit) return false;
  }
  return true;
}
