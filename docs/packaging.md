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
