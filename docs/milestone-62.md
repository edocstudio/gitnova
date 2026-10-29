# M62 — Exact-commit GitLab pipeline in the graph

The existing CI column between Commit message and Author now shows the latest GitLab pipeline state and number, for example `✓ Passed · #40778`. Its tooltip names the pipeline ref and last-check time. The existing commit context menu opens that pipeline through the validated external-link handler. No new screen, inspector section, connection form or per-row interactive control is needed; the graph remains the context and Settings → Connections retains host account configuration. English and Thai labels preserve original refs and IDs.

The lightweight commit read already used by graph CI now consumes `last_pipeline` when complete. It validates the returned commit ID, pipeline SHA, numeric ID and optional canonical web URL against the selected project. Older responses without `web_url` receive a constructed exact-project pipeline route. A pipeline belonging to an MR source commit is never painted onto a different merge commit. Unknown states remain unknown; commit-level status remains a fallback when no identified pipeline is returned. Inaccessible CI cannot imply success. GitHub combined statuses/checks retain their existing behavior.

No jobs, logs or artifacts are requested for graph rows. Existing visible-commit limits, serialized provider reads, bounded cache, quiet refresh and hidden-window pause remain. Matching global host credentials are still required for private repositories; a dash without a connection explains that requirement on hover. No private company API or real account credentials were used for verification.

See [verification](verification/2026-10-29-graph-pipeline.md) for the scoped checks and their limits.
