# License evidence for local alpha artifacts

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
