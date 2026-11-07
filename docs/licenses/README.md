# License evidence for local artifacts

GitNova's own source remains Apache-2.0; the standard LICENSE appendix is
unchanged. NOTICE records project attribution, not a completed dependency audit.

`dependency-inventory.json` is an offline inventory of the locked Rust graph for
`aarch64-apple-darwin` with `gitnova-desktop/custom-protocol`, and the npm lockfile
with installed package evidence. It includes development/build and optional
candidates; it is **not** an exact list of code shipped in the executable or JS.
Assets are hashed candidates requiring provenance/permission review, including
documentation screenshots that are not automatically included in the app.

`third-party-license-evidence.txt` preserves collected license/notice texts,
deduplicated by SHA-256, with their package/version and original relative paths.
It supplements `terminal-components.txt`. It does not substitute for missing
texts, license-choice decisions, source obligations or asset permission review.

Reproduce after installing the locked development dependencies:

```bash
python3 apps/desktop/scripts/inventory-licenses.py --target aarch64-apple-darwin
```

Collection reads cached sources without downloading dependencies or executing
package scripts. Missing sources/texts and component-specific terms are recorded
explicitly. Review changes to all evidence files; do not mark `auditComplete`
true merely because collection or packaging succeeds. For another target,
perform a separate inventory/review instead of reusing this target's evidence.

`supplemental-evidence.json` records upstream texts omitted from a packaged
crate. Each entry pins the published crate's VCS revision, declared license,
original source URL and text hash. Collection fails if those values disagree;
it never downloads a replacement implicitly. The entries supply the repository-root BSD text for `alloc-stdlib` 0.3.0,
the original MIT text for `dlopen2` 0.8.2 / `dlopen2_derive` 0.4.3 and the MIT
option for `libappindicator-sys` 0.9.0 at their published VCS revisions.
The Windows `webview2-com` 0.39.1, `webview2-com-sys` 0.39.1 and
`webview2-com-macros` 0.8.1 entries preserve the original repository-root MIT
texts at each crate's published revision. These are Rust binding texts, not
approval of WebView2 runtime/SDK redistribution.
`selectors` 0.38.0 explicitly declares MPL 2.0 in its pinned source header;
its entry supplies the canonical text from Mozilla, the license steward.
This does not resolve MPL source-form obligations or bundled native libraries.
The current macOS inventory still has 49 unresolved entries and 27
component-specific terms to review.
For example, the [pinned objc2 upstream licensing document](https://github.com/madsmtm/objc2/blob/d7d2fa23ceaa5e6096c923b081040e5d81b3b9df/LICENSE.md) discusses generated
Apple SDK bindings; collecting that document alone would not resolve their
redistribution terms. Missing texts, SDK terms and actual shipped material
remain review items, not inferred passes.

The local macOS app includes project LICENSE, NOTICE, terminal attribution,
the inventory and collected texts under `Contents/Resources/licenses/`.
`build:local` checks lockfile hashes before building and byte equality of all five
resources afterward. These are inclusion/integrity checks, not legal approval.
Direct verification:

```bash
node apps/desktop/scripts/verify-bundle-licenses.mjs /path/to/GitNova.app
```

See the [distribution checklist](../contributing/binary-distribution.md) and
[M83 verification](../verification/2026-10-30-distribution-evidence.md) for the
remaining public distribution gates.

Windows/Linux native packaging uses a separate inventory generated for the runner host and verifies all five attribution resources in the installed NSIS app, extracted Debian package and extracted AppImage. See [desktop platforms](../platforms.md). These inclusion checks do not resolve the inventory’s audit gaps or certify redistribution compliance.

AppImage also contains native Linux libraries outside the Cargo/npm graph.
Its upstream package copyright files can reference `/usr/share/common-licenses`.
The Linux packaging configuration includes that build-host directory at the
same path inside AppImage using [Tauri's custom file mapping](https://v2.tauri.app/distribute/appimage/#custom-files).
The Ubuntu packaging host must provide it. Installed-artifact verification
checks that each referenced common text exists inside the extracted image and
records hashes in `appimage-native-notices.json`; missing or escaping files fail
the candidate job. This checks text inclusion, not license selection,
corresponding-source/relinking obligations or complete native-library compliance.

See [unsigned alpha candidates](../contributing/unsigned-alpha.md) for installed
startup checks, artifact hashes and the deferred signing procedure.
