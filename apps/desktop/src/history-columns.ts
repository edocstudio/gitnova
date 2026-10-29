export const HISTORY_COLUMNS = ['refs', 'graph', 'message', 'ci', 'author', 'date'] as const;
export type HistoryColumn = typeof HISTORY_COLUMNS[number];
export type HistoryWidths = Partial<Record<HistoryColumn, number>>;
export const HISTORY_WIDTHS_KEY = 'gitnova.history-column-widths.v1';
const limits: Record<HistoryColumn, [number, number]> = {refs:[100,600], graph:[64,1600], message:[180,1800], ci:[80,480], author:[80,600], date:[80,400]};
export function columnLimits(column: HistoryColumn, naturalGraph: number): [number,number] {
  // Keep circles and branch bends readable; compress lane spacing, never crop lanes.
  const min = column === 'graph' ? Math.max(64, Math.ceil((naturalGraph - 24) / 2) + 24) : limits[column][0];
  return [min, Math.max(min, limits[column][1])];
}
export function columnWidth(column: HistoryColumn, value: number, naturalGraph: number) {
  const [min,max] = columnLimits(column,naturalGraph);
  return Math.round(Math.max(min,Math.min(max,Number.isFinite(value)?value:min)));
}
export function parseHistoryWidths(raw: string|null): HistoryWidths {
  try {
    const value: unknown = JSON.parse(raw??'{}');
    if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
    const result: HistoryWidths = {};
    for (const column of HISTORY_COLUMNS) {
      const width=(value as Record<string,unknown>)[column];
      if(typeof width==='number' && Number.isFinite(width)) result[column]=columnWidth(column,width,88);
    }
    return result;
  } catch {return {};}
}
export function historyColumnLayout(widths: HistoryWidths, naturalGraph: number) {
  const defaults={refs:174,graph:Math.min(naturalGraph,360),message:180,ci:140,author:128,date:104};
  const values=Object.fromEntries(HISTORY_COLUMNS.map(column=>[column,columnWidth(column,widths[column]??defaults[column],naturalGraph)])) as Record<HistoryColumn,number>;
  const fixedMessage=widths.message!==undefined;
  const template=HISTORY_COLUMNS.map(column=>column==='message'&&!fixedMessage?`minmax(${values.message}px,1fr)`:`${values[column]}px`).join(' ')+(fixedMessage?' minmax(0,1fr)':'');
  return {values,template,minWidth:Object.values(values).reduce((sum,n)=>sum+n,14)};
}
