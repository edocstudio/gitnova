# GLib 0.18.5 backport

GitNova's Linux GTK3/WebKit dependencies require the GLib 0.18 API. The workspace Cargo patch resolves that package to the official crates.io 0.18.5 archive with upstream commit [b5a4071e439b](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34) applied to `src/variant_iter.rs`. Upstream uses a mutable out-parameter for the GLib string iterator. The archive SHA-256 is `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`. The upstream license and copyright are included in the package directory.

Remove this patch when the Linux native dependency graph can use upstream GLib 0.20 or later. Track the local package separately when scanning registry lockfile entries. [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
