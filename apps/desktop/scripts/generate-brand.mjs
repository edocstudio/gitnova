// Code-native vector master. Run before `npm run tauri -- icon assets/icon.svg`.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = new URL('../', import.meta.url);
const symbol = `<g fill="none" stroke-linecap="round" stroke-linejoin="round">
  <path d="M16 14C7 20 5 31 10 40c4 8 12 12 22 12" stroke="currentColor" stroke-width="3.5"/>
  <path d="M48 50c9-6 11-17 6-26-4-8-12-12-22-12" stroke="currentColor" stroke-width="3.5"/>
</g>
<path d="M32 14c2.5 11 7 15.5 18 18-11 2.5-15.5 7-18 18-2.5-11-7-15.5-18-18 11-2.5 15.5-7 18-18Z" fill="currentColor"/>
<circle cx="16" cy="14" r="4.5" fill="currentColor"/>
<circle cx="48" cy="50" r="4.5" fill="currentColor"/>`;
const mark = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64" color="#4b91ff">${symbol}</svg>\n`;
writeFileSync(new URL('assets/logo-mark.svg', root), mark);
writeFileSync(new URL('assets/logo-mark-mono.svg', root), mark.replace('color="#4b91ff"', 'color="#ffffff"'));
const defs = `<defs>
 <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#1d3448"/><stop offset=".52" stop-color="#10202f"/><stop offset="1" stop-color="#0b1723"/></linearGradient>
 <linearGradient id="light" x1="0" y1="0" x2=".85" y2="1"><stop stop-color="#527b98" stop-opacity=".7"/><stop offset=".55" stop-color="#2c465a" stop-opacity=".5"/><stop offset="1" stop-color="#182d3e"/></linearGradient>
 <linearGradient id="nova" x1=".15" y1="0" x2=".8" y2="1"><stop stop-color="#9bd4ff"/><stop offset=".4" stop-color="#5ba4ff"/><stop offset="1" stop-color="#3775e7"/></linearGradient>
</defs>`;
const tile = `<rect x="8" y="8" width="112" height="112" rx="27" fill="url(#tile)" stroke="url(#light)" stroke-width="1.5"/>`;
// The same silhouette is used in the Dock, app header and flat/one-color mark.
const icon = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 128 128">${defs}${tile}<g transform="translate(22 22) scale(1.3125)" color="#4b91ff">${symbol.replaceAll('stroke="currentColor"','stroke="url(#nova)"').replaceAll('fill="currentColor"','fill="url(#nova)"')}</g></svg>\n`;
writeFileSync(new URL('assets/icon.svg', root), icon);
const iconBody=icon.slice(icon.indexOf('>')+1,icon.lastIndexOf('</svg>'));
const board=`<svg xmlns="http://www.w3.org/2000/svg" width="1440" height="1000" viewBox="0 0 1440 1000">
<rect width="1440" height="1000" fill="#0d1721"/>
<g font-family="-apple-system,BlinkMacSystemFont,Inter,Arial,sans-serif">
 <text x="72" y="68" fill="#8aa1b5" font-size="14" letter-spacing="3">GITNOVA / ORBITAL NOVA</text>
 <svg x="104" y="142" width="350" height="350" viewBox="0 0 128 128">${iconBody}</svg>
 <text x="536" y="282" fill="#e4edf5" font-size="90" font-weight="650" letter-spacing="-4">GitNova</text>
 <text x="542" y="340" fill="#9db5cb" font-size="29">Git made simple.</text>
 <text x="542" y="386" fill="#718da5" font-size="22">Every repository. Every change.</text>
 <path d="M72 558H1368" stroke="#253a4b"/>
 <text x="92" y="618" fill="#8aa1b5" font-size="13" letter-spacing="2">ONE MARK. EVERY SIZE.</text>
 ${[16,24,32,64,128].map((size,i)=>`<svg x="${92+i*106}" y="${682+(128-size)/2}" width="${size}" height="${size}" viewBox="0 0 128 128">${iconBody}</svg><text x="${92+i*106}" y="846" fill="#718da5" font-size="12">${size} PX</text>`).join('')}
 <path d="M688 612V872" stroke="#253a4b"/>
 <svg x="750" y="678" width="104" height="104" viewBox="0 0 64 64" color="#4b91ff">${symbol}</svg>
 <svg x="944" y="678" width="104" height="104" viewBox="0 0 64 64" color="#e4edf5">${symbol}</svg>
 <rect x="1131" y="650" width="140" height="160" rx="18" fill="#edf3f9"/>
 <svg x="1149" y="678" width="104" height="104" viewBox="0 0 64 64" color="#2764cb">${symbol}</svg>
 <text x="750" y="846" fill="#718da5" font-size="12">FLAT BLUE</text><text x="944" y="846" fill="#718da5" font-size="12">ONE COLOR</text><text x="1134" y="846" fill="#718da5" font-size="12">LIGHT SURFACE</text>
 <text x="92" y="950" fill="#718da5" font-size="14">Nova light. Git paths. An original identity for a local-first developer tool.</text>
</g></svg>`;
writeFileSync(new URL('../../docs/brand/orbital-nova.svg',root),board);
console.log(`Generated vector assets from ${fileURLToPath(import.meta.url)}`);
