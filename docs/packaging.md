# macOS packaging

The product name is **GitRonimo**. The crate is still `gitronimo-desktop`; the binary and `.app` executable are `GitRonimo` so the macOS application menu title matches.

Product version **2.0.1** is the string shown in About GitRonimo (`APP_VERSION` in `apps/desktop/src/views/about.rs`) and in the bundle (`[package.metadata.packager] version` in `apps/desktop/Cargo.toml`). Bump **both** after each release. They are independent of the Cargo workspace crate version.

Local `.app` bundles are unsigned. Gatekeeper will not treat them as a distributable release.

The app identifier is `com.gitronimo.desktop`. The dock and Finder icon is `assets/gitronimo.icns`, generated from `icon.png`. About GitRonimo shows `assets/gitronimo-icon.png`. Local unsigned builds: `./bin/build`. Keep certificate material, team IDs, and notary credentials in the release environment or CI secrets; never in this repository.

## Local `.app`

```bash
./bin/build
```

The script installs `cargo-packager 0.11.8` if needed, builds the native architecture, and writes `target/release-arm/GitRonimo.app` (Apple Silicon) or `target/release-intel/GitRonimo.app` (Intel).

## Apple Silicon (arm64)

```bash
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"
cargo build --release -p gitronimo-desktop
cargo install cargo-packager --version 0.11.8 --locked
cargo packager --release --formats app \
  --manifest-path apps/desktop/Cargo.toml \
  --out-dir "$(pwd)/target/release-arm" \
  --binaries-dir "$(pwd)/target/release"
open target/release-arm/GitRonimo.app
```

Use absolute `--out-dir` / `--binaries-dir` values. cargo-packager resolves relative paths from `apps/desktop/Cargo.toml`.

Confirm the executable architecture:

```bash
lipo -archs target/release-arm/GitRonimo.app/Contents/MacOS/GitRonimo
# arm64
```

## Intel (x86_64)

Cross-compile from Apple Silicon (or build natively on Intel):

```bash
rustup target add x86_64-apple-darwin
cargo build --release -p gitronimo-desktop --target x86_64-apple-darwin
cargo packager --release --formats app --target x86_64-apple-darwin \
  --manifest-path apps/desktop/Cargo.toml \
  --out-dir "$(pwd)/target/release-intel" \
  --binaries-dir "$(pwd)/target/x86_64-apple-darwin/release"
lipo -archs target/release-intel/GitRonimo.app/Contents/MacOS/GitRonimo
# x86_64
```

## Zip artifacts

```bash
mkdir -p target/dist
ditto -c -k --sequesterRsrc --keepParent \
  target/release-arm/GitRonimo.app \
  target/dist/GitRonimo-2.0.1-macos-arm64.zip
ditto -c -k --sequesterRsrc --keepParent \
  target/release-intel/GitRonimo.app \
  target/dist/GitRonimo-2.0.1-macos-x86_64.zip
(cd target/dist && shasum -a 256 GitRonimo-2.0.1-macos-*.zip > SHA256SUMS.txt)
```

A universal binary (lipo of both executables into one `GitRonimo.app`) is produced by `.github/workflows/release.yml` on a `v*` tag, then signed and notarized.

## Signing and notarization

For a distributable release, import an Apple Developer ID Application certificate into a dedicated keychain, then sign with hardened runtime and timestamping:

```bash
codesign --force --deep --options runtime --timestamp --sign "Developer ID Application: YOUR NAME (TEAMID)" target/release-arm/GitRonimo.app
codesign --verify --deep --strict --verbose=2 target/release-arm/GitRonimo.app
xcrun notarytool submit YOUR_DMG_OR_ZIP --keychain-profile "notary-profile" --wait
xcrun stapler staple target/release-arm/GitRonimo.app
```

## Protected CI release

Pushing a `v*` tag starts `.github/workflows/release.yml`. Configure these repository secrets before creating the tag:

- `DEVELOPER_ID_APPLICATION`: the Developer ID Application signing identity name.
- `MACOS_CERTIFICATE_BASE64`: the base64-encoded `.p12` certificate.
- `MACOS_CERTIFICATE_PASSWORD`: the `.p12` password.
- `KEYCHAIN_PASSWORD`: a unique temporary CI keychain password.
- `APPLE_API_KEY_BASE64`: the base64-encoded App Store Connect API key `.p8` file.
- `APPLE_API_KEY_ID` and `APPLE_API_ISSUER`: the corresponding App Store Connect API key identifiers.

The workflow builds arm64 and x86_64 bundles, creates a universal app, signs it with hardened runtime and a timestamp, notarizes and staples it (retries `notarytool --wait` on App Store Connect HTTP timeouts), writes `SHA256SUMS.txt`, and publishes the ZIP with `CHANGELOG.md` as the release notes. If a GitHub release for that tag already exists, the workflow uploads the ZIP onto it instead of failing.

If `notarytool` reports `The request timed out` after `Successfully uploaded file`, signing already succeeded; re-run the job. Apple sometimes takes 15–40 minutes, and a poll timeout is not a certificate problem.

Unsigned per-architecture CI artifacts are also uploaded from `.github/workflows/ci.yml` (`GitRonimo-unsigned-macos-arm64` and `GitRonimo-unsigned-macos-x86_64`). The in-app updater refuses those: it only installs a zip whose extracted `GitRonimo.app` passes `codesign --verify --deep --strict` and `spctl --assess --type execute`.

## In-app updates

**GitRonimo → Settings…** **In-app updates** is on by default. There is no check on launch. **Check now** (About **Check for updates**, GitRonimo menu **Check for Updates…**, palette **Check for updates**) GETs `https://api.github.com/repos/mpakus/gitronimo/releases/latest` without a PAT, downloads `SHA256SUMS.txt` then `GitRonimo-${tag}.zip`, verifies SHA-256 with `shasum -a 256`, unpacks with `ditto`, Gatekeeper-assesses the `.app`, then replaces the running `GitRonimo.app` (backup beside it, restored on copy failure). `cargo run` / `target/` binaries are not a bundle and are refused. Failed hash or Gatekeeper leaves the running app untouched. No telemetry.
