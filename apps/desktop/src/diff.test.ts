import { describe, expect, it } from 'vitest';
import { parseDiff, splitDiffLines } from './diff';
describe('patch line navigation', () => {
  it('keeps old/new line numbers correct through multiple hunks and files', () => {
    const files = parseDiff('diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -4,2 +4,3 @@\n keep\n-old\n+new\n+extra\n@@ -10 +11 @@\n-last\n+next\ndiff --git a/new file.txt b/new file.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new file.txt\n@@ -0,0 +1 @@\n+created\n');
    expect(files.map(f => f.name)).toEqual(['src/a.ts', 'new file.txt']);
    expect(files[0].additions).toBe(3); expect(files[0].deletions).toBe(2);
    expect(files[0].lines.find(l => l.text === 'old')).toMatchObject({ old: 5, next: null });
    expect(files[0].lines.find(l => l.text === 'next')).toMatchObject({ old: null, next: 11 });
    expect(files[1].lines.at(-1)).toMatchObject({ next: 1, old: null, text: 'created' });
  });
  it('represents binary changes without inventing text lines', () => {
    const files = parseDiff('diff --git a/image.png b/image.png\nindex 123..456\nBinary files a/image.png and b/image.png differ\n');
    expect(files[0].lines).toEqual([]); expect(files[0].metadata).toContain('Binary files a/image.png and b/image.png differ');
  });
});

describe('side-by-side alignment', () => {
  it('aligns replacement runs and leaves a real empty cell for unmatched lines', () => {
    const patch = parseDiff('diff --git a/a b/a\n@@ -4,3 +4,4 @@\n same\n-old\n+new\n+extra\n end\n')[0];
    const rows = splitDiffLines(patch.lines);
    expect(rows.find(row => row.left?.text === 'old')?.right?.text).toBe('new');
    const extra = rows.find(row => row.right?.text === 'extra');
    expect(extra?.left).toBeNull(); expect(extra?.right?.next).toBe(6);
    expect(rows.find(row => row.left?.text === 'end')).toMatchObject({ left: { old: 6 }, right: { next: 7 } });
  });
  it('keeps replacements aligned across a no-newline marker', () => {
    const patch = parseDiff('diff --git a/a b/a\n@@ -1 +1 @@\n-before\n\\ No newline at end of file\n+after\n\\ No newline at end of file\n')[0];
    const rows = splitDiffLines(patch.lines);
    expect(rows.find(row => row.left?.text === 'before')?.right?.text).toBe('after');
    expect(rows.filter(row => row.left?.kind === 'meta' || row.right?.kind === 'meta')).toHaveLength(2);
  });
});

describe('Git path identity', () => {
  const native = (hex: string, display = '') => ({ hex, display, encoding: 'unix' });
  it('separates Git marker tabs from real filename spaces without trimming the name', async () => {
    const { findPatch } = await import('./diff');
    const files = parseDiff('diff --git a/space name  b/space name \n--- a/space name \t\n+++ b/space name \t\n@@ -1 +1 @@\n-before\n+after\n');
    expect(files[0].name).toBe('space name ');
    expect(findPatch(files, native('7370616365206e616d6520'))).toBe(0);
  });
  it('decodes quoted controls and octal UTF-8 while retaining raw non-UTF-8 identity', async () => {
    const { findPatch } = await import('./diff');
    const files = parseDiff('diff --git "a/quote\\\"\\t\\n.txt" "b/quote\\\"\\t\\n.txt"\n--- "a/quote\\\"\\t\\n.txt"\n+++ "b/quote\\\"\\t\\n.txt"\n@@ -0,0 +1 @@\n+one\ndiff --git "a/\\377.txt" "b/\\377.txt"\nBinary files differ\ndiff --git "a/\\376.txt" "b/\\376.txt"\nBinary files differ\n');
    expect(files[0].name).toBe('quote"\t\n.txt');
    expect(files[1].name).toBe(files[2].name);
    expect(findPatch(files, native('ff2e747874', '�.txt'))).toBe(1); expect(findPatch(files, native('fe2e747874', '�.txt'))).toBe(2);
  });
  it('never interprets hunk content as a file header', () => {
    const files = parseDiff('diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n--- a/not-a-path\n+++ b/not-a-path\n');
    expect(files[0].name).toBe('a.txt'); expect(files[0].lines.at(-1)?.text).toBe('++ b/not-a-path');
  });
  it('handles header-only binary paths containing the header delimiter', () => {
    expect(parseDiff('diff --git a/a b/name b/a b/name\nBinary files differ\n')[0].name).toBe('a b/name');
    expect(parseDiff('diff --git "a/old\\tname" "b/new\\tname"\nsimilarity index 100%\nrename from "old\\tname"\nrename to "new\\tname"\n')[0].name).toBe('new\tname');
  });
  it('rejects missing and ambiguous path identities instead of choosing a different file', async () => {
    const { findPatch } = await import('./diff'); const patch = parseDiff('diff --git a/a.txt b/a.txt\nBinary files differ\n');
    expect(findPatch(patch, native('622e747874'))).toBe(-1); expect(findPatch([...patch, ...patch], native('612e747874'))).toBe(-1);
  });
});

it('does not count the native unmerged preamble as another changed file', () => {
  const files = parseDiff('* Unmerged path conflict.txt\ndiff --git a/conflict.txt b/conflict.txt\n--- a/conflict.txt\n+++ b/conflict.txt\n@@ -1 +1 @@\n-old\n+new\n');
  expect(files).toHaveLength(1); expect(files[0].name).toBe('conflict.txt');
});
