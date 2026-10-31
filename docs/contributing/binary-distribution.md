# Before distributing a GitNova binary

The current owner-approved preparation path is an [unsigned alpha candidate](unsigned-alpha.md), with installed startup evidence and a separate future signing procedure. This does not complete the checklist below.

This is an incomplete release checklist, not a declaration of license compliance or release readiness. GitNova remains alpha 0.1.0. The current `npm run build:local` creates a local macOS ad-hoc-signed debug app; it does not create a notarized public release or complete this audit automatically. It checks locked-inventory freshness and inclusion/integrity of the five attribution evidence resources described in [license evidence](../licenses/README.md).

[LICENSE](../../LICENSE) contains Apache-2.0, including its standard appendix. [NOTICE](../../NOTICE) contains GitNova project attribution. Dependency and asset notices may impose additional obligations; the current NOTICE is not a completed third-party inventory. Review the actual release contents and applicable licenses rather than inferring compliance from a crate's headline license field.

## License and attribution review

- [ ] Record the exact source commit, versions, target OS/architecture, enabled features, lockfiles and build/package commands used for the artifact.
- [ ] Inventory direct and transitive Rust dependencies, including target/feature-specific, native and build components that contribute material to the distribution. Inspect their actual license files, copyright statements and notice requirements.
- [ ] Inventory npm dependencies and the generated frontend bundle, including transitive components and any bundled code/assets. Distinguish shipped content from tools used only during development; document that assessment.
- [ ] Review fonts, icons, images, screenshots, generated brand assets and included sample/source data for origin, permission and attribution requirements. A claim of original artwork is not a third-party asset audit.
- [ ] Determine applicable license choices and obligations for the shipped components; preserve required copyright notices, license texts and NOTICE content. Investigate unknown/incompatible terms before distribution. Assess any required source or other accompanying materials instead of assuming Apache-2.0 covers dependencies.
- [ ] Prepare the release's third-party notices/license texts and include GitNova LICENSE and applicable NOTICE content. Do not replace the standard Apache appendix's example copyright line as if it were a project placeholder.
- [ ] Confirm whether Git or other external executables/runtimes are bundled. GitNova currently expects an externally installed Git; bundling it would require a separate licensing and packaging review.

## Verify the distributed artifact

- [ ] Inspect the built `.app`, archive or installer, not only the source tree: verify all required notices/license texts and accompanying materials are actually included and accessible to recipients. The local build script checks the five recorded attribution resources byte for byte, but cannot establish that the inventory includes every required notice or that legal obligations are fulfilled.
- [ ] Check for unintended credentials, environment files, proprietary fixtures, personal paths and private recovery/catalog data in the artifact and accompanying materials.
- [ ] Record the inventory, unresolved questions, required notices, inclusion locations and verification evidence for each shipped target. Do not mark the audit complete while any required component or asset remains unreviewed.
- [ ] Describe the build accurately: debug versus release profile, tested platforms, signing identity and notarization status. Ad-hoc signing and successful local verification do not establish Developer ID signing, notarization or native acceptance on every OS.

See [dependency choices](../architecture/dependencies.md), [contributor checks](../../CONTRIBUTING.md#run-relevant-checks) and [current platform status](../../README.md#project-status). This checklist does not add a CLA, DCO or contributor sign-off requirement.
