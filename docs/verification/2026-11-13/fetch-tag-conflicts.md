# Fetch with existing local tags

Version: 1.0.13. Verification platform: macOS arm64, Apple Git 2.39.2,
Rust 1.99.1. All Git mutations used disposable repositories and local remotes;
no company repository, credential or remote was used.

## Cause and change

`GitCli::fetch_remote_impl` previously combined `--tags` with `--atomic`.
A remote tag with the same name but a different object caused Git to reject
the fetch, preventing remote-tracking branch updates too.

Fetch now explicitly maps remote heads and tags, excluding all existing local
tag names using negative refspecs. Refspecs are supplied through stdin to avoid
command-line size limits. No second network request or force-tag update is
introduced. Manual Fetch, Auto fetch and branch-fetch callers share this path.
Existing protocol restrictions, credentials bridge, timeout, atomic ref updates,
nonrecursive submodule policy and preservation of FETCH_HEAD remain in place.

The local tag is intentionally retained even when it differs from the server.
Remote graph tag inspection remains separate. If another process creates a
conflicting tag after inspection, Git can still reject the atomic transaction;
this fix does not retry or overwrite concurrent external changes.

## Verification

- Debug core `remotes`: 14 passed.
- Release core `remotes`: 14 passed.
- Release core `tags`: 7 passed after updating the previous test contract that
  explicitly expected a tag conflict to fail Fetch.
- Regression coverage includes lightweight and annotated conflicting tags,
  new tag import, repeated fetch, selected/all branch fetch, Auto fetch, branch
  switching/Pull validation, denied custom transports and real fetch failure.
- Fixtures assert preservation of HEAD, original tags, index bytes, staged and
  unstaged work, untracked files, Git config and FETCH_HEAD.
- Browser tests with real Rust core: 2 passed. Header Fetch refreshes the
  incoming count despite a conflicting local tag, preserves dirty work and
  imports a new tag. Remote-only annotated graph tags remain visible before
  Fetch and deduplicate after import. The browser transport is emulated.
- HTTPS credentials bridge integration: 1 passed with a local TLS fixture and
  synthetic credentials, including Fetch after Push and unchanged Git config.
- Version synchronization script: 1 passed.
- Native macOS release build completed; TypeScript check and Vite build passed.
  Five bundled attribution resources, embedded UI assets and ad-hoc signature
  verified. Installed over Desktop/GitNova.app while closed; installed version
  and bundle verification both report 1.0.13. Executable SHA-256:
  `1b2bd5fa581b1f318f4cc944cbe66f8450687d28fb4e84d0c7773dda44568fe6`.
- `git diff --check` passed.

Windows/Linux native execution and live provider authentication are not
certified by these local fixture tests. No remote push or release publication.
