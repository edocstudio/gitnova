# Security policy

## Alpha support scope

GitNova is currently an alpha, version 0.1.0. This policy describes the current development branch (`main`) and does not establish a supported stable/LTS series or a backport commitment for older alpha snapshots. Include the exact version/commit in a report. No response-time SLA, guaranteed fix timeline or production-support commitment is established. Native desktop validation and local debug packaging have focused on macOS Apple Silicon; Linux/Windows core CI configuration is not native desktop certification.

## Reporting a suspected vulnerability

Do not publish credentials, private repository contents, exploit instructions or details of an unpatched vulnerability in a public issue, discussion or pull request. Logs, screenshots, recovery folders and Git configuration can contain sensitive data. Keep evidence local until a private channel has been agreed.

As of 2026-10-29, an unauthenticated check of this repository's GitHub private-vulnerability-reporting endpoint returned `enabled: false`. No maintainer-approved private email or alternative reporting URL has been verified for this policy. **A private vulnerability-report destination is not currently available through this document.**

To request a private contact route, open a [GitHub issue](https://github.com/edocstudio/gitnova/issues/new) containing only a non-sensitive request such as “Please provide a private security contact.” Do not include the affected feature, vulnerability details, credentials or proof of concept in that public request. This issue route is for requesting contact, not for submitting the vulnerability itself. Wait for a maintainer to establish a private channel before sending details.

GitHub private reporting is separate from adding SECURITY.md. This file does not enable it or claim that it is enabled. See [GitHub's reporting guidance](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately). The reporting instructions must be updated once the maintainer confirms a working private destination.

## Information for an agreed private report

Provide the GitNova version/commit, OS and Git version, affected operation, expected versus observed behavior, and a minimal reproduction using disposable repositories and synthetic data. Describe potential impact and relevant pending-operation/recovery state. Redact tokens, proprietary source, personal data and private URLs; a recovery archive may contain original files and should not be attached wholesale.

Local Git does not require a provider account. Optional macOS provider/Jenkins credentials are stored in Keychain; local catalog metadata and recovery evidence have different storage and sensitivity properties. See the [architecture](docs/architecture/overview.md#product-boundary) for the implemented boundaries. Do not use real credentials, workplace repositories or another person's data to reproduce a suspected issue.
