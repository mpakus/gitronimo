# Security policy

Do not file public issues for vulnerabilities that could expose repository contents or credentials, permit unintended Git execution, or bypass destructive-action confirmation.

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/mpakus/gitronimo/security/advisories/new) on the `mpakus/gitronimo` repository. Include a minimal reproduction, affected version, impact, and safe remediation guidance. Do not include real credentials or private repository data.

GitRonimo does not collect telemetry by default and does not upload crash reports. Git authentication remains with the installed Git credential helper and SSH configuration; GitRonimo does not implement a credential store in the 1.0.0 release.

When a GitHub personal access token is connected from **Settings**, it is stored only in the macOS Keychain under GitRonimo's service name. It is not written to preferences, activity messages, diagnostics, crash reports, Git arguments, or repository files. GitHub API requests are made only to load the connected account and the pull requests requested by the user. GitHub responses are not uploaded or used for telemetry. Disconnecting an account removes the Keychain item on a best-effort basis and reports a failure if macOS refuses the removal.
