# Version tags and hosted requests remain graph-context actions

Status: proposed; 2026-10-27 user design request.

Use the selected graph commit for Create tag and the exact selected branch for
Create PR/MR. Extend the current lightweight-tag entry and existing review surface,
rather than adding a permanent release or requests dashboard. Add contextual
Push tag because current branch Push deliberately excludes tags and cannot
express publishing one immutable tag object. Show hosted request detail in the
existing inspector/diff dock.

Local tag creation, remote publication, request creation and hosted merge are
separate confirmed operations with separate receipts and partial outcomes.
Provider source/target and account/host identity must be explicit. Git transport
credentials do not imply provider API authentication. Keep Tauri-independent
provider adapters and native OS credential storage; no always-on cache service.

The reviewable UX, authentication prerequisites, recovery behavior, phased scope
and real-Git/provider acceptance criteria are specified in
[version-tags-and-change-requests.md](../design/version-tags-and-change-requests.md).
