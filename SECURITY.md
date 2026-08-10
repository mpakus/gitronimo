# Security policy

Do not file public issues for vulnerabilities that could expose repository contents or credentials, permit unintended Git execution, or bypass destructive-action confirmation.

Until a dedicated security contact is published, use the repository host's private vulnerability-reporting channel. Include a minimal reproduction, affected version, impact, and safe remediation guidance. Do not include real credentials or private repository data.

Gitronimo does not collect telemetry by default and does not upload crash reports. Git authentication remains with the installed Git credential helper and SSH configuration; Gitronimo does not implement a credential store in the beta.

When the optional GitHub Services view is used, the personal access token is stored only in the macOS Keychain under Gitronimo's service name. It is not written to preferences, activity messages, diagnostics, crash reports, Git arguments, or repository files. GitHub API requests are made only to load the connected account and the repositories or pull requests requested by the user. GitHub responses are not uploaded or used for telemetry. Disconnecting an account removes the Keychain item on a best-effort basis and reports a failure if macOS refuses the removal.
