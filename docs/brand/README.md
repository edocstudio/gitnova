# GitNova — Orbital Nova

An original vector identity combining a four-point nova with two open commit
orbits. The star is the point where paths meet; circular nodes keep the connection
to Git visible. The identity uses original geometric artwork.

Keep the established blue `#4b91ff` and deep navy `#10202f`. The app tile uses a
restrained blue highlight and navy surface; the flat mark works in one color.
The paired open arcs, diagonally opposed nodes and centered star form the same
silhouette at every size. No glow, animated asset or runtime dependency is used.

- [Identity board](orbital-nova.png), [vector board](orbital-nova.svg)
- Vector master: `apps/desktop/scripts/generate-brand.mjs`
- Transparent blue/white symbols: `apps/desktop/assets/logo-mark*.svg`
- Desktop/app tile: `apps/desktop/assets/icon.svg`
- Native macOS/Windows/PNG exports: `apps/desktop/src-tauri/icons/`

The header and favicon load the same source SVG used to generate the native app
icon. Git operation icons remain functional UI symbols. The icon is decorative
beside the existing accessible GitNova brand label.

Regenerate SVGs from the repository root with
`node apps/desktop/scripts/generate-brand.mjs`. Export desktop raster/ICNS/ICO
assets using the installed Tauri icon CLI with that generated icon.svg, and copy
its top-level desktop outputs into src-tauri/icons (mobile outputs are not used).
The PNG identity board is a browser rendering of the vector board.

Wordmark and tagline remain: GitNova / Git made simple. / Every repository. Every
change. The lettering on the board uses the system sans-serif stack; it is not a
new font dependency or a claim of trademark clearance.

Original identity verification: viewed the native 128px PNG and the size/monochrome board; rendered
the built app header and checked that its image and favicon load. TypeScript,
Vite and local macOS native build passed, as did signing/embedded-asset checks
for the then-current local bundle. That historical alternate bundle is retired;
the current builder updates only `target/debug/bundle/macos/GitNova.app`.

[Built app header](app-header.png).

Header refinement: the logo footprint is now 44px (40px at narrow widths), and
the footer credits GitNova · by edocstudio. Current-branch/Pull/Push controls
share the repository context row; global workspace navigation and search stay
in the header. The existing Branch menu aligns inward at narrow widths.

Verified the existing real-Git Fetch/Pull flow and menu bounds at 1360/1000/780px,
plus the multiple-remote flow and 26 frontend checks.
[Wide layout](repository-toolbar-1360.png), [narrow layout](repository-toolbar-780.png).

## Repository introduction

[README hero](readme-hero.svg) reuses the original flat logo paths and established navy/blue palette with an abstract branch illustration. It is a self-contained SVG with accessible title/description, no external font/image dependency and no app runtime changes. The main README screenshots are fresh browser captures of the actual UI against disposable Git fixtures.
