# Publication privacy review — 2026-10-28

## Scope

Reviewed the current files selected by `git ls-files --cached --others --exclude-standard` before the first commit. The initial inventory contained 382 publishable files: 283 text files and 99 binary assets. No local commits or refs existed at review time. This document is an additional text file after that inventory.

The review covers the current publication set, not ignored build artifacts, an external application's data directory, future commits or unknown upstream repositories.

## Checks and changes

- Searched publication text for known organization/employee identifiers, user-specific absolute paths, non-example contacts, internal remote hosts, credential-bearing URLs, private-key headers and common access-token formats. Reviewed matches in test code as synthetic rejection fixtures, including deliberately invalid URLs and placeholder credentials.
- Replaced four user-specific build-result paths with `/path/to/gitnova`.
- Replaced the local freeze record with an engineering summary, excluding raw workstation process/security-product inventory and account-specific details. Originals were retained privately outside this repository.
- Ran English OCR over all 97 PNG assets successfully and searched recognized text for the reviewed identifiers, home paths and credential signatures. No such matches remained. Reviewed reference/hosting screenshots directly; displayed accounts and projects are test fixtures.
- Checked PNG text/EXIF chunks: none were present. Inspected the generated ICNS and ICO containers (12 blocks and six entries) for embedded PNG metadata. SVG assets were included in the text review.
- Confirmed that the publication set contained no environment files, signing/private-key files, runtime credentials/catalogs/receipts or private review archives. Existing build/test output remains ignored.
- Extended root ignore rules for local credential directories/files, root PEM/key files and a private-local directory.

## Before committing

The explicitly approved copyright name and edocstudio branding remain public. Git author/committer identity comes from Git configuration and is separate from source files; check the effective identity locally before the first public commit. Choose a personal public address or the exact noreply address supplied by the hosting provider. Never put private identity values in this report.

These checks found no remaining reviewed private identifiers or credential signatures in the publication set. Pattern matching and OCR are bounded inspections, not a guarantee that every possible secret or piece of identifying information can be recognized. Recheck newly staged files and any new screenshots before publication.

## M50 source recheck

After adding first-project publication, the eligible publication inventory contained 389 files. Rechecked text for the previously reviewed private identifiers and home paths: no matches. The effective local author and committer match the user's explicitly supplied public name and GitHub noreply address. The actual project still has zero commits, zero configured remotes and zero staged paths; publication tests used only disposable repositories. New test screenshots and local build logs remain ignored/outside the publication set. This recheck does not replace the image review scope above.
