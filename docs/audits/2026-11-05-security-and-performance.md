# Security and performance review — 2026-11-05

Base: `70fb8cb` (`feat: browse connected repositories and improve submodule workflows`).
The checkout was clean before this review. This is a targeted review of the current
alpha, not a penetration test or a claim that the application has no vulnerabilities.

## Dependency evidence and unresolved risks

Machine-readable results and lockfile hashes are in
[security-dependencies.json](2026-11-05-security-dependencies.json).

- `npm audit --json`: zero reported advisories across the locked frontend dependency
  tree, including development dependencies. No automatic fixes were applied.
- Queried all 536 public registry package/version pairs in `Cargo.lock` against the
  [OSV batch API](https://google.github.io/osv.dev/post-v1-querybatch/). Six batches,
  no remaining pagination tokens. No private package paths or credentials sent.
  This is an OSV lookup, not a `cargo audit` execution.
- **Unresolved Linux dependency issue:** `glib 0.18.5` is affected by
  [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html), an unsound
  `VariantStrIter` implementation. `GHSA-wrw7-89jp-8q8g` is an alias of the same issue.
  The fixed upstream line starts at 0.20.0. `cargo tree --locked --workspace --target
  x86_64-unknown-linux-gnu -i glib --depth 4` confirms the GTK3/WebKit/Tauri chain uses
  0.18.5. A compatible single-package update cannot cross GTK's 0.18 constraint.
  A reviewed backport or coordinated upstream migration and Linux native regression
  verification remains necessary. No suppression, forced incompatible upgrade or
  claim of non-exploitability was added. This GTK dependency is not used by the
  macOS native backend; that does not establish whole-platform security.
- Four **maintenance advisories**, distinct from demonstrated exploit vulnerabilities:
  [proc-macro-error](https://rustsec.org/advisories/RUSTSEC-2024-0370.html),
  [zvariant](https://rustsec.org/advisories/RUSTSEC-2026-0370.html),
  [zbus_names](https://rustsec.org/advisories/RUSTSEC-2026-0371.html), and
  [zvariant_derive](https://rustsec.org/advisories/RUSTSEC-2026-0372.html).
  Multiple locked versions share the same advisories; they are not separate issues.
  Replacing the Linux secure-store/desktop dependency chain needs compatibility
  testing; these dependencies were not silently replaced in this patch.
- The local Git executable reports `2.39.2 (Apple Git-143)`. Git is outside Cargo/npm
  auditing. GitNova's feature minimum is not a security baseline. Check vendor
  security updates before handling untrusted repositories; no system tools were
  installed or upgraded here. Version alone cannot establish Apple backport status.
  See the [Git project's vulnerability announcement](https://github.blog/open-source/git/git-security-vulnerabilities-announced-6/).

## Source boundaries inspected

Reviewed native IPC/CSP/capabilities, Git credential routing, provider HTTP clients,
URL validation, terminal ownership/output limits, file-edit path validation and
resource cleanup. Existing controls include credential-free validated HTTPS links,
redirect-disabled bounded provider responses, exact provider/host matching,
OS credential stores, scoped Git helpers, bounded terminal queues and guarded
working-file writes. These observations do not certify every caller or every OS.
A read-only pattern scan of 531 tracked text files found no matches for the checked
GitHub/GitLab token formats, AWS access-key IDs or private-key headers. Binary files
were excluded; this does not inspect Git history, local application state or every
possible secret format.

No real credentials were read to test these boundaries, and no provider/Git writes
were sent to a network destination.

## Changes

- CI summary previously validated the same Git remote/commit twice and looked up
  credentials twice. A private scope resolver now validates the selected remote
  once; CI reads retrieve credentials once. Explicit account connection does not
  read the old secret before prompting for the replacement. Summary requests still
  verify the exact configured host before credential lookup. No credential cache,
  longer secret lifetime, widened host scope or changed HTTP cache TTL was added.
- `NativePath` encoding now preallocates its hex output and uses a digit lookup
  rather than allocating a formatted string for every byte. Unix encoding also
  borrows the original bytes instead of copying them first. Decoding validates byte
  pairs directly. Input over 256 KiB of encoded hex is rejected before decoding;
  NUL paths are rejected at the OS code-unit level. Non-UTF-8 Unix paths and Windows
  UTF-16 code units remain lossless. The old decoder already rejected non-ASCII
  input: this is hardening and allocation reduction, not a newly discovered panic.
  The size guard bounds decoding, not initial JSON deserialization.

## Validation

- `npm audit --json`: exit 0, zero reported vulnerabilities.
- `cargo test --locked -j 1 -p gitnova-domain -- --test-threads=1`: three native-path tests passed on macOS. The Windows-only UTF-16 test is present but was not executed here.
- `cargo test --locked -j 1 -p gitnova-core --lib pipeline_scope_tests -- --test-threads=1`: one new CI scope regression passed using a disposable repository, without a credential-store read.
- `cargo test --locked -j 1 -p gitnova-core --test read_optimization -- --test-threads=1`: three existing read/cache regressions passed.
- `cargo test --locked -j 1 -p gitnova-hosting --lib -- --test-threads=1`: 41 passed, one isolated OS credential-store test intentionally ignored.
- `cargo clippy --locked -j 1 -p gitnova-core -p gitnova-domain --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed. Local links in the touched documentation resolve.
- `npm run build:local` with `MACOSX_DEPLOYMENT_TARGET=13.0`: passed, including frontend type checking/production build and native bundle verification. The existing main JS chunk size warning remains; this patch does not claim to solve initial UI load size.
- Updated the existing Desktop `GitNova.app` after the user closed it. Installed bundle signature and embedded assets verified; installed executable matches the built executable. SHA-256: `0423d8ec02df70199094842305c9a57008f20e05f3cb20fdcc93cfaaefbf9996`. This is a local debug/ad-hoc-signed build, not a notarized release. No actual repository window or authenticated operation was launched for verification.

[Release microbenchmark samples](../benchmarks/native-path-2026-11-05.json)
compare the original encoder with the new encoder: five alternating samples of
5,000 encodings per implementation, after warmup. Median duration for 64-character
paths decreased from 16.809 ms to 0.876 ms; for 1,024-character paths from 213.237 ms
to 6.728 ms on this machine. These are **path encoding timings only**, not whole-app
speedups, Git operation timings, measured allocation counts or RAM stability.
Reproduce with `cargo run --locked --release -j 1 -p gitnova-domain --example path_encoding_bench`.
No historical benchmark baseline or website snapshot was overwritten.

## Remaining limits

No new Windows/Linux native execution, system WebView audit, real-token flow,
full penetration test or long-duration whole-app RAM experiment was performed.
The targeted changes do not establish that branch switching, network latency or
whole-app memory growth have been solved. The Linux glib advisory remains open.
