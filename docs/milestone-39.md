# M39 — Direct local and remote branch deletion

The graph context menu now names Delete local branch and Delete remote branch
separately. Local deletion already existed inside Manage branch; the new direct
entry opens that existing preview, preserving occupancy checks and recovery.
Current/linked-worktree checked-out branches cannot be deleted. Local and remote
removal are separate confirmations; neither implicitly performs the other.

Remote deletion inspects one configured push destination and the live branch OID.
The selected cached OID must match. Preview never executes push hooks. The
advertised default branch, mirror remotes, custom fetch mappings, multiple push
URLs, unavailable local recovery objects and same-repository destinations block
confirmation. Existing destination credential/transport constraints also apply.
Different fetch/push URLs are explicit in the review; their fetch cache stays
unchanged. Server protections are authoritative and cannot be bypassed here.

Before publication, preserve the exact commit under a deterministic local recovery
ref; expose it in the review and recorded activity. Send only a deletion refspec
with an explicit expected old OID. This uses Git's
[exact force-with-lease form](https://git-scm.com/docs/git-push)
as compare-and-delete, without unrestricted force or automatic retry. A changed
remote tip is refused. After verifying server absence, remove only the reviewed
matching direct tracking ref with expected-old-OID validation. Local heads, index,
files and upstream configuration stay in place. A network/cache race can leave
partial effects; its receipt remains unknown with live-remote/cache/recovery
inspection guidance. No automatic rollback or recreation is attempted.

Coverage uses temporary real repositories and bare remotes: clean/dirty/staged
preservation, unrelated refs, exact recovery, default branch refusal, stale
remote/configuration, pre-push race, separate push URL, mirror/custom mapping and
same-repository refusal. Browser coverage checks direct menu discovery, remote
confirmation, direct local confirmation and checked-out-local blockers, alongside
the existing management regression. EN/TH labels share the existing graph review.
Live GitHub/GitLab protected-branch administration is not changed or simulated as
acceptance; normal configured Git authentication and server policy apply.

Verification completed: 5 Core real-Git integration cases, 2 browser flows
(new direct deletion plus existing ref management), 26 frontend tests and
TypeScript checks passed. The native Tauri real-Git/origin integration test also
passed, including external-origin rejection for both new remote-delete commands.

[Remote deletion review in a temporary repository](screenshots/remote-branch-delete-review.png).

The local macOS artifact `target/debug/bundle/macos/GitNova Updated.app` passed
TypeScript/native build, ad-hoc signature and embedded-asset verification. It
uses the Updated name to avoid overwriting the running GitNova bundle.
