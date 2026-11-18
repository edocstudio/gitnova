# Verified Windows installer handoff — 1.3.1

Windows CI ran on 2026-11-18; final documentation completed 2026-11-19 (Asia/Bangkok).

The real Windows x64 release-profile NSIS installer is available locally at:

`target/windows-handoff/1.3.1-734d364/GitNova_1.3.1_x64-setup.exe`

The same folder contains `SHA256SUMS`, `WINDOWS-DOWNLOAD.md`, and exact native,
lifecycle and website handoff metadata. No public asset URL, tag, GitHub Release
or website deployment was created. macOS's installed app remains 1.3.0.

| Identity | Verified value |
| --- | --- |
| Version / target | 1.3.1 / x86_64-pc-windows-msvc |
| Build source | `734d36405fa446e55471cb8ee3e945c02f4afb46` |
| Branch / base | `build/windows-installer` / main `81a67ab` |
| Installer bytes | 5,695,521 |
| Installer SHA-256 | `d57f808fc1ee89a94b6ee94291a2d39a2611291ebedd2b59413cf1acd19708e6` |
| GitHub artifact | 14448103720, `gitnova-windows-handoff-734d36405fa446e55471cb8ee3e945c02f4afb46` |
| Archive SHA-256 | `04fc193ea0b933fbea7b9607a8bd3906b8225b34a2dc2e8bf00c71ad923a823a` |
| Signing | Unsigned candidate |

## Passing Windows verification

[Run 44339604949](https://github.com/edocstudio/gitnova/actions/runs/44339604949)
passed every required Windows job for the exact source above:

| Gate | Result |
| --- | --- |
| Core formatter, Clippy and workspace tests, job 134925906245 | 473 passed, 0 failed, 2 existing ignored; completed in about 58 minutes including setup |
| Browser/native shard 1, job 134925906312 | 43 browser workflows; native IPC/PTY; frontend; credential store; actual installer lifecycle passed |
| Browser/native shard 2, job 134925905879 | 43 browser workflows passed |
| Browser/native shard 3, job 134925906256 | 43 browser workflows passed |
| Browser/native shard 4, job 134925906260 | 43 browser workflows passed |
| Exact artifact handoff, job 134970807876 | Passed |

There are 172 browser workflows total, 109 unique frontend unit tests across 26
files, and eight native Windows IPC/PTY tests. Shards repeat shared native/unit
checks; these counts are not multiplied. The two ignored core cases are an internal
child-process fixture and an OS credential round-trip explicitly run separately
in the isolated Windows Credential Manager step.

The actual NSIS installer passed installation, installed embedded UI/native
catalog IPC/real-Git graph, single-instance reuse, preserved fixture HEAD/working
tree, same-version reinstall and uninstall. Five attribution resources were
verified after install and reinstall. Local retrieval then checked the GitHub
archive digest, safe ZIP entries, PE executable header, exact bytes/size/hash,
source/version/run identity and every SHA256SUMS entry. The NSIS bootstrap is
32-bit PE; the verified application payload/target is x64.

See [check summary](checks.json), [download verification](download-verification.json),
[original native report](windows-artifact.json),
[original lifecycle report](windows-lifecycle.json) and
[website handoff metadata](windows-handoff.json).

## Corrections needed to obtain this result

Windows browser harness fixes scope the Thai Remote selector, freeze/drain
background snapshots before exact write-refresh counts, normalize positive and
negative worktree path assertions, and use 45-second Windows expectations with a
180-second test budget. All expected states, tests and product write guards remain.

First candidate `af7209f`,
[run 44319555944](https://github.com/edocstudio/gitnova/actions/runs/44319555944),
passed all four browser shards and built NSIS, but native startup failed eager
user-data path resolution in the isolated profile. Correction `995a2d5` lazily
uses the validated fixture catalog and creates redirected profile directories.
Its [run 44329598401](https://github.com/edocstudio/gitnova/actions/runs/44329598401)
passed installed startup/native IPC/real Git/single-instance, then hit Windows
EPERM removing the temporary profile after process exit. Correction `6f9d8d0`
uses bounded recursive cleanup retries (20 with 100ms linear backoff, about
21 seconds total); persistent cleanup failures still fail.

Prior passing Windows core job 134610046069 took about 86 minutes/469 tests, close
to the old 90-minute cap. Correction `734d364` uses two Windows core test threads
within AGENTS limits and a 120-minute cap. Other OS/core and desktop worker
settings are unchanged. No test was skipped or safety invariant weakened.
The obsolete runs were cancelled to release branch concurrency; unfinished core
and shard results from them are incomplete, not passed. Run 44337962235 was stopped
for this final CI-budget correction. Only the final run supplies this handoff.

Local checks also passed: five new integrity tests; 11 combined packaging script
tests plus one existing Linux GPG skip; both workflow YAML parses; three affected
real-core macOS browser cases; formatter; 11 native macOS desktop tests; smoke
JavaScript syntax; diff checks. These local checks do not substitute for Windows.

## Handoff to gitnova-web and limits

Give the complete local folder to gitnova-web; use `WINDOWS-DOWNLOAD.md` and the
manifest for exact asset metadata. Keep public URL fields null until reviewed
publication of this exact installer and SHA256SUMS under a matching version tag,
then verify anonymous downloads before changing the website feed. Actions artifact
URLs expire/require access and are not public installer links. Existing website
macOS/Linux packages retain their original versions and metadata.

This candidate is unsigned. The Windows inventory reports `auditComplete: false`
and 56 unresolved entries; packaging does not approve the existing license audit.
Manual GUI/provider-account acceptance and migration from a previous installed
version are separate. Linux and macOS hosted acceptance is outside this Windows
run. No public release or site deployment is claimed.
