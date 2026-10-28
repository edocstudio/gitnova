import type { NativePath } from './contracts';
export interface DiffLine { kind: 'context' | 'add' | 'remove' | 'hunk' | 'meta'; text: string; old: number | null; next: number | null }
export interface PatchFile { name: string; pathHex: string | null; lines: DiffLine[]; additions: number; deletions: number; metadata: string[] }
const encoder = new TextEncoder();
type PathToken = { bytes: number[]; end: number };
function pathToken(text: string): PathToken | null {
  if (!text.startsWith('"')) return { bytes: [...encoder.encode(text.split('\t')[0])], end: text.length };
  const bytes: number[] = []; let i = 1;
  const escapes: Record<string, number> = { a: 7, b: 8, t: 9, n: 10, v: 11, f: 12, r: 13, '\\': 92, '"': 34 };
  while (i < text.length) {
    const ch = text[i++];
    if (ch === '"') return { bytes, end: i };
    if (ch === '\\') {
      const escape = text[i++];
      if (escape in escapes) bytes.push(escapes[escape]);
      else if (/[0-7]/.test(escape ?? '')) {
        let octal = escape;
        for (let n = 1; n < 3 && /[0-7]/.test(text[i] ?? ''); n++) octal += text[i++];
        const value = Number.parseInt(octal, 8); if (value > 255) return null; bytes.push(value);
      } else return null;
    } else {
      const point = text.codePointAt(i - 1)!; const char = String.fromCodePoint(point);
      if (char.length === 2) i++; bytes.push(...encoder.encode(char));
    }
  }
  return null;
}
function setPath(file: PatchFile, text: string, prefixed: boolean) {
  const token = pathToken(text); if (!token) return;
  let bytes = token.bytes;
  if (new TextDecoder().decode(new Uint8Array(bytes)) === '/dev/null') return;
  if (prefixed) {
    if (bytes[1] !== 47 || (bytes[0] !== 97 && bytes[0] !== 98)) return;
    bytes = bytes.slice(2);
  }
  file.name = new TextDecoder().decode(new Uint8Array(bytes));
  file.pathHex = bytes.map(byte => byte.toString(16).padStart(2, '0')).join('');
}
function headerPath(file: PatchFile, text: string) {
  if (text.startsWith('"')) {
    const first = pathToken(text); if (!first) return;
    const second = text.slice(first.end + 1);
    setPath(file, second, true); return;
  }
  // Unquoted names may contain spaces and even ' b/'. Prefer a split whose paths agree.
  for (let index = text.indexOf(' b/'); index >= 0; index = text.indexOf(' b/', index + 1)) {
    if (text.slice(2, index) === text.slice(index + 3)) { setPath(file, text.slice(index + 1), true); return; }
  }
  // Rename/copy headers below supply the authoritative destination for ambiguous names.
}
export function findPatch(files: PatchFile[], path: NativePath): number {
  const matches = files.flatMap((file, index) => (path.encoding === 'unix' ? file.pathHex === path.hex : file.name === path.display) ? [index] : []);
  return matches.length === 1 ? matches[0] : -1;
}
export function parseDiff(raw: string): PatchFile[] {
  const files: PatchFile[] = []; let file: PatchFile | undefined; let old = 0; let next = 0; let inHunk = false;
  for (const line of raw.split('\n')) {
    // Native --ours diff announces the unmerged entry before its actual patch.
    if (!file && line.startsWith('* Unmerged path ')) continue;
    if (line.startsWith('diff --git ')) {
      file = { name: 'Unrecognized path', pathHex: null, lines: [], additions: 0, deletions: 0, metadata: [] };
      headerPath(file, line.slice(11)); files.push(file); inHunk = false; continue;
    }
    if (!file) { if (!line) continue; file = { name: 'Changes', pathHex: null, lines: [], additions: 0, deletions: 0, metadata: [] }; files.push(file); }
    if (!inHunk && (line.startsWith('--- ') || line.startsWith('+++ '))) { setPath(file, line.slice(4), true); continue; }
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (hunk) { old = Number(hunk[1]); next = Number(hunk[2]); inHunk = true; file.lines.push({ kind: 'hunk', text: line, old: null, next: null }); continue; }
    if (!inHunk) {
      if (line.startsWith('rename to ') || line.startsWith('copy to ')) setPath(file, line.slice(line.startsWith('rename') ? 10 : 8), false);
      if (line && !line.startsWith('index ')) file.metadata.push(line); continue;
    }
    if (line.startsWith('+')) { file.additions++; file.lines.push({ kind: 'add', text: line.slice(1), old: null, next: next++ }); }
    else if (line.startsWith('-')) { file.deletions++; file.lines.push({ kind: 'remove', text: line.slice(1), old: old++, next: null }); }
    else if (line.startsWith(' ')) file.lines.push({ kind: 'context', text: line.slice(1), old: old++, next: next++ });
    else if (line) file.lines.push({ kind: 'meta', text: line, old: null, next: null });
  }
  return files;
}

export interface SplitDiffLine { left: DiffLine | null; right: DiffLine | null }
/** Pair adjacent removal/addition runs without changing either side's line numbers. */
export function splitDiffLines(lines: DiffLine[]): SplitDiffLine[] {
  const rows: SplitDiffLine[] = [];
  let removed: DiffLine[] = []; let added: DiffLine[] = []; let markers: SplitDiffLine[] = [];
  const flush = () => {
    for (let i = 0; i < Math.max(removed.length, added.length); i++) rows.push({ left: removed[i] ?? null, right: added[i] ?? null });
    rows.push(...markers); removed = []; added = []; markers = [];
  };
  for (const line of lines) {
    if (line.kind === 'remove') { if (added.length) flush(); removed.push(line); }
    else if (line.kind === 'add') added.push(line);
    else if (line.kind === 'meta' && (removed.length || added.length)) {
      // A no-newline marker belongs to its preceding side; it must not split a replacement run.
      const right = added.length > 0;
      markers.push({ left: right ? null : line, right: right ? line : null });
    } else { flush(); rows.push({ left: line, right: line }); }
  }
  flush(); return rows;
}
