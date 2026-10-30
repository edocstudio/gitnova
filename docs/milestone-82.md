# M82 — committed line attribution and root-package evidence

These are bounded read-only investigations in the existing Search context.
Neither fetches, checks out files, executes package tools/scripts or changes refs.

## Line attribution

Choose **Line attribution (blame)**, a literal regular file, an exact local
revision, and an inclusive range of at most 200 lines. This mode uses Current
repository; other history/comparison modes retain their workspace scopes.
Revision resolution is bound to an OID and a refs/config snapshot. The committed
file must be UTF-8 text, contain no NUL and fit within 1 MiB. The range end is
clamped to the committed file length; an empty/outside range is refused.

Native `git blame --line-porcelain --root --no-textconv` provides original OIDs,
line numbers and historical filenames. Results navigate to the graph/diff at the
original path. Rename attribution follows native Git heuristics; this does not
enable copied/moved-line `-C`/`-M` analysis or working-content attribution. Shallow
boundaries are explicitly incomplete. Configured ignore-revisions files are
refused before blame: the tested Git version can open its configured file before
processing a later empty CLI override. Unsupported/non-UTF-8 historical filenames
receive an explicit diagnostic; no fabricated path is substituted.

## Root package.json facts

Branch comparison now reads the committed root package.json at both exact tips,
showing names, versions and changed dependencies/devDependencies/peerDependencies/
optionalDependencies. Files are regular, at most 64 KiB; declared and changed
requirements each have a 100-entry bound. Scripts and lockfiles are not executed
or resolved. Root workspaces receive an explicit unverified-coverage notice.

A disposable request-local lookup inspects at most 32 explicitly registered
locations under a five-second admission budget. It validates repository identity,
reads a committed root manifest at an observed local OID and checks refs again.
Multiple worktrees retain their locations; multiple repository identities claiming
the same package name are ambiguous. Missing/failed/bounded lookups stay unknown.
Alias, local-path, workspace and Git constraints are not resolved or mapped to a
repository merely by a similar name. The shared lookup avoids rescanning all
registered repositories for every comparison location.

A matched name and observed local version do **not** establish semver satisfaction,
lockfile resolution, registry publication, CI success or release readiness.
Monorepo subpackages and non-package.json ecosystems need additional adapters;
there is no general package/release certification claim.

## Navigation correction and evidence

Opening an investigation result that is already selected now explicitly refreshes
commit details. Previously clearing details without changing the selected OID
could leave the inspector indefinitely at “Reading…”. The browser regression
covers both this path and navigation to an earlier commit/historical filename.

See [verification](verification/2026-10-30-archaeology-packages.md) and the
[Git blame manual](https://git-scm.com/docs/git-blame#_the_porcelain_format).
