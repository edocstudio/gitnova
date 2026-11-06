# Direct build links from graph CI badges

Known pipeline and single-workflow badges reuse CiRunLink and the existing
validated native browser opener. They link to the exact provider build URL, not
the commit page or a URL inferred from a folder name. Click/keyboard events do not
bubble into graph selection or checkout handling. Empty/unknown CI has no invented
link; aggregate workflows keep their existing named context-menu choices.

Checks executed on 2026-11-06:

- TypeScript check passed.
- Frontend unit suite: 68 passed in 17 files.
- Targeted browser CI integration: 1 passed. Covered direct Running/Passed GitLab
  pipeline links, Enter activation, direct GitHub workflow links, retained context
  menu navigation, host isolation, late results and visibility pausing. Git data
  used temporary repositories and real Rust; provider responses and browser opening
  were intercepted, with no company credentials or live company endpoints used.
- React review: reuse the existing small link component, event-driven opening,
  native anchor keyboard behavior, bounded existing CI data, no new polling/effect
  or dependencies, existing EN/TH labels.

This does not certify live-provider access or cross-platform native browser launch.

Local macOS build passed; installed over the closed canonical Desktop GitNova.app.
Strict/deep signature and installed `--verify-bundle` passed. Executable SHA-256
matches the build:
`8fb5575792dfd482be881b431954354688a25064ec0817c784b19639875ede3c`.
The existing debug/ad-hoc signing, absent notarization, shared-chunk warnings and
incomplete attribution inventory remain unchanged. `git diff --check` passed.
No commit, push or release was performed.
