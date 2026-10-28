import type { CSSProperties } from 'react';
type Name = 'settings' | 'trash' | 'search' | 'tag' | 'remote' | 'branch' | 'chevron' | 'refresh' | 'folder' | 'check' | 'close' | 'plus' | 'arrow' | 'file' | 'commit' | 'layers' | 'lock' | 'back' | 'star' | 'copy' | 'panel';
const paths: Record<Name, React.ReactNode> = {
  settings: <><polygon points="18.929,9.130 19.332,10.421 21.877,10.436 21.877,13.564 19.332,13.579 18.929,14.870 18.929,14.870 18.301,16.068 20.090,17.878 17.878,20.090 16.068,18.301 14.870,18.929 14.870,18.929 13.579,19.332 13.564,21.877 10.436,21.877 10.421,19.332 9.130,18.929 9.130,18.929 7.932,18.301 6.122,20.090 3.910,17.878 5.699,16.068 5.071,14.870 5.071,14.870 4.668,13.579 2.123,13.564 2.123,10.436 4.668,10.421 5.071,9.130 5.071,9.130 5.699,7.932 3.910,6.122 6.122,3.910 7.932,5.699 9.130,5.071 9.130,5.071 10.421,4.668 10.436,2.123 13.564,2.123 13.579,4.668 14.870,5.071 14.870,5.071 16.068,5.699 17.878,3.910 20.090,6.122 18.301,7.932 18.929,9.130"/><circle cx="12" cy="12" r="3.2"/></>,
  trash: <><path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"/></>,
  search: <><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/></>,
  tag: <><path d="M3 3h8l10 10-8 8L3 11Z"/><circle cx="7.5" cy="7.5" r="1"/></>,
  remote: <><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 21h8m-4-5v5"/></>,
  copy: <><rect x="8" y="8" width="12" height="13" rx="2"/><path d="M16 8V3H3v13h5"/></>,
  panel: <><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></>,
  branch: <><circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="5" r="2"/><path d="M6 7v10M18 7c0 7-12 3-12 10"/></>,
  chevron: <path d="m8 10 4 4 4-4"/>, refresh: <><path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 7a7 7 0 0 1 11.7-1L20 9M4 15l2.2 3A7 7 0 0 0 18 17"/></>,
  folder: <path d="M3 7V5h6l2 2h10v12H3V7Z"/>, check: <path d="m5 12 4 4L19 6"/>, close: <path d="m6 6 12 12M6 18 18 6"/>, plus: <path d="M12 5v14M5 12h14"/>, arrow: <path d="M5 12h14m-5-5 5 5-5 5"/>,
  file: <><path d="M14 3H5v18h14V8l-5-5Z"/><path d="M14 3v5h5"/></>, commit: <><circle cx="12" cy="12" r="4"/><path d="M3 12h5m8 0h5"/></>, layers: <><path d="m12 3 10 5-10 5L2 8l10-5ZM2 12l10 5 10-5M2 16l10 5 10-5"/></>, lock: <><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></>, back: <path d="M19 12H5m5-5-5 5 5 5"/>, star: <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9L12 3Z"/>,
};
export function Icon({ name, size = 16, className, style }: { name: Name; size?: number; className?: string; style?: CSSProperties }) {
  return <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" className={className} style={style}>{paths[name]}</svg>;
}
