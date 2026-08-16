# Security policy

Do not file public issues for vulnerabilities that could expose repository contents or credentials, permit unintended Git execution, or bypass destructive-action confirmation.

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/mpakus/gitronimo/security/advisories/new) on the `mpakus/gitronimo` repository. Include a minimal reproduction, affected version, impact, and safe remediation guidance. Do not include real credentials or private repository data.

GitRonimo does not collect telemetry by default and does not upload crash reports. Git authentication remains with the installed Git credential helper and SSH configuration; GitRonimo does not implement a general-purpose credential vault.

When a GitHub personal access token is connected from **Settings**, it is stored only in the macOS Keychain under service `com.gitronimo.github`. It is not written to preferences, activity messages, diagnostics, crash reports, Git arguments, or repository files. GitHub API requests are made only to load the connected account and the pull requests requested by the user. In-app update checks use the public GitHub Releases API without that token. GitHub responses are not uploaded or used for telemetry. Disconnecting an account removes the Keychain item on a best-effort basis and reports a failure if macOS refuses the removal.

When **AI commit messages** are on, an OpenAI-compatible API key (Settings **API key…**) is stored only in Keychain under service `com.gitronimo.ai-commit` (account `default`), separate from the GitHub token. Suggest sends the redacted staged diff to the configured endpoint; it does not send the GitHub PAT, the full repository, or unredacted Git output. The suggestion fills the commit composer and never creates a commit by itself. Clearing the key removes that Keychain item on a best-effort basis.
