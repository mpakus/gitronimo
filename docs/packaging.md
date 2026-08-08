# macOS packaging

The development bundle is intentionally unsigned. Build it on macOS with:

```bash
cargo build --release -p gitronimo-desktop
cargo install cargo-packager --version 0.11.8 --locked
cargo packager --release --formats app --manifest-path apps/desktop/Cargo.toml --out-dir target/release
open target/release/Gitronimo.app
```

The app identifier is `com.gitronimo.desktop`. The generated `Gitronimo.app` is suitable for local technical validation only; Gatekeeper will not treat it as a distributable release.

For a release, import an Apple Developer ID Application certificate into a dedicated keychain, then sign the bundle with hardened runtime and timestamping:

```bash
codesign --force --deep --options runtime --timestamp --sign "Developer ID Application: YOUR NAME (TEAMID)" target/release/Gitronimo.app
codesign --verify --deep --strict --verbose=2 target/release/Gitronimo.app
xcrun notarytool submit YOUR_DMG_OR_ZIP --keychain-profile "notary-profile" --wait
xcrun stapler staple target/release/Gitronimo.app
```

Keep certificate material, team IDs, and notary credentials in the release environment or CI secrets; never in this repository.

## Protected CI release

Pushing a `v*` tag starts `.github/workflows/release.yml`. Configure these repository secrets before creating the tag:

- `DEVELOPER_ID_APPLICATION`: the Developer ID Application signing identity name.
- `MACOS_CERTIFICATE_BASE64`: the base64-encoded `.p12` certificate.
- `MACOS_CERTIFICATE_PASSWORD`: the `.p12` password.
- `KEYCHAIN_PASSWORD`: a unique temporary CI keychain password.
- `APPLE_API_KEY_BASE64`: the base64-encoded App Store Connect API key `.p8` file.
- `APPLE_API_KEY_ID` and `APPLE_API_ISSUER`: the corresponding App Store Connect API key identifiers.

The workflow builds arm64 and x86_64 bundles, creates a universal app, signs it with hardened runtime and a timestamp, notarizes and staples it, runs Gatekeeper assessment, writes `SHA256SUMS.txt`, and publishes the ZIP with `CHANGELOG.md` as the release notes.
