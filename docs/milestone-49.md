# M49 — Optional local source branch cleanup after merge

The existing graph merge review gains an unchecked-by-default option naming the exact local source branch to delete after success. The finish review repeats the selected scope. No new screen, navigation item or separate dialog is needed: this is part of the existing source-to-destination merge confirmation. Remote refs, raw commits, cherry-picks and reverts cannot select this option. The user explicitly chose local branches only.

The choice is included in the preview token and owned-operation marker, survives reopening a pending merge, and clears the old preview when toggled. Older requests/markers default to keeping the source. A merge abort or unresolved conflict never starts cleanup. Fast-forward cleanup runs only after the target tip is verified; two-parent merge cleanup runs only after the reviewed commit is verified.

Cleanup is a second, separately journaled operation. It checks the current destination/merged OID, ancestry, exact unchanged source OID and worktree/configuration state, then reuses the existing local ref deletion transaction and recovery ref. A moved source, source checked out in another worktree or rejected delete is preserved or reported as uncertain as appropriate. The successful merge is never rolled back or relabeled as failed. Its receipt links the cleanup status, separate receipt and recovery ref; the existing error surface shows a separate cleanup warning. There is no automatic retry after an uncertain result. A crash between merge completion and cleanup can leave the source branch intact; there is no claim of atomic merge-and-delete.

All Git mutations and hook failure injection in verification use disposable repositories. EN/TH strings accompany the checkbox, confirmation and result/warning text.

## Repository ignore rules

Expanded the root `.gitignore` for Rust bundles, frontend dependencies/generated assets, reports/coverage, local environment files, signing credentials and OS/editor temporary files. Verified ten representative generated/private paths are ignored and seven representative source/configuration/template paths remain eligible, including Cargo/package lockfiles and `tauri.local.conf.json`.

## Verification

- Seven focused real-repository checks passed: opt-in deletion after finish; fast-forward; default keep/abort; newer unmerged source tip; linked-worktree refusal; token/source/operation scope; rejected native reference transaction with merge success retained. Source recovery refs and unchanged remote-tracking refs are checked explicitly.
- All seven preceding cherry-pick/submodule preservation checks passed again after adding the request/state fields.
- TypeScript, all 32 frontend unit tests, strict Git/core/desktop Clippy (all targets) and formatting passed.
- Two single-worker browser flows backed by the real core passed: default-unchecked checkbox through successful local cleanup, and merge success with newer source preserved plus a visible separate cleanup warning. Visual inspection at 1360×900 confirmed the checkbox and scope fit the existing graph review. The browser transport is emulated; no live developer repository was written.
- Local debug bundle built in 1m 07s and signing/embedded assets verified: `target/debug/bundle/macos/GitNova Updated.app` (79.11 MiB). The main GitNova bundle was in use, so the build selected the separate Updated bundle. No GUI was launched and the running app was left in place.
