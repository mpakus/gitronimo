//! Download, verify, and replace a notarized `GitRonimo.app` from GitHub Releases.
//!
//! Git and GPUI stay out of this module. Network uses typed `curl` arguments.
//! Unsigned or `Gatekeeper`-rejected bits are never installed.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use hosting_github::{download_url_is_allowed, sha256_for_filename};

const SUMS_MAX_BYTES: u64 = 1_048_576;
const ZIP_MAX_BYTES: u64 = 157_286_400;

/// Release assets the user confirmed installing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingAppUpdate {
    pub version: String,
    pub zip_name: String,
    pub zip_url: String,
    pub sums_url: String,
}

/// `GitRonimo.app` that contains this executable, if we are running from a bundle.
#[must_use]
pub(crate) fn bundle_from_executable(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    if macos.file_name() != Some(OsStr::new("MacOS")) {
        return None;
    }
    if contents.file_name() != Some(OsStr::new("Contents")) {
        return None;
    }
    if bundle.file_name() != Some(OsStr::new("GitRonimo.app")) {
        return None;
    }
    Some(bundle.to_path_buf())
}

/// Download, hash-check, `Gatekeeper`-assess, and replace the running `.app`.
///
/// # Errors
/// Returns a user-facing sentence. Failed hash or `Gatekeeper` leaves the running
/// bundle untouched. Staging files are deleted on both success and failure.
pub(crate) fn install_release(
    pending: &PendingAppUpdate,
    current_exe: &Path,
) -> Result<(), String> {
    let bundle = bundle_from_executable(current_exe).ok_or_else(|| {
        "Could not install the update: in-app updates only replace a GitRonimo.app bundle (not cargo run)."
            .to_owned()
    })?;
    if !hosting_github::is_safe_asset_filename(&pending.zip_name) {
        return Err("Could not install the update: the zip name is not a release asset.".into());
    }
    let staging = std::env::temp_dir().join(format!("gitronimo-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|_| "Could not create a staging folder for the update.".to_owned())?;
    let result = install_into_staging(pending, &staging, &bundle);
    let _ = fs::remove_dir_all(&staging);
    result
}

fn install_into_staging(
    pending: &PendingAppUpdate,
    staging: &Path,
    bundle: &Path,
) -> Result<(), String> {
    let sums_path = staging.join("SHA256SUMS.txt");
    let zip_path = staging.join(&pending.zip_name);
    curl_download(&pending.sums_url, &sums_path, SUMS_MAX_BYTES)?;
    let sums_text =
        fs::read_to_string(&sums_path).map_err(|_| "Could not read SHA256SUMS.txt.".to_owned())?;
    let expected = sha256_for_filename(&sums_text, &pending.zip_name).map_err(|_| {
        "Could not install the update: SHA256SUMS.txt is missing the zip hash.".to_owned()
    })?;
    curl_download(&pending.zip_url, &zip_path, ZIP_MAX_BYTES)?;
    let actual = file_sha256_hex(&zip_path)?;
    if actual != expected {
        return Err(
            "Could not install the update: the download did not match SHA256SUMS.txt.".into(),
        );
    }
    let extract = staging.join("extract");
    let staged_app = extract_gitronimo_app(&zip_path, &extract)?;
    assess_gatekeeper(&staged_app)?;
    replace_application_bundle(&staged_app, bundle)
}

fn curl_download(url: &str, dest: &Path, max_bytes: u64) -> Result<(), String> {
    if !download_url_is_allowed(url) {
        return Err("Could not download the update: the URL is not a GitHub release asset.".into());
    }
    let status = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--max-filesize",
            &max_bytes.to_string(),
            "--header",
            "User-Agent: GitRonimo",
            "--output",
        ])
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|_| "Could not start the update download.".to_owned())?;
    if status.success() {
        Ok(())
    } else {
        Err("Could not download the update from GitHub Releases.".into())
    }
}

fn file_sha256_hex(path: &Path) -> Result<String, String> {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .map_err(|_| "Could not verify the download hash.".to_owned())?;
    if !output.status.success() {
        return Err("Could not verify the download hash.".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout
        .get(..64)
        .ok_or_else(|| "Could not verify the download hash.".to_owned())?;
    if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Could not verify the download hash.".into());
    }
    Ok(hash.to_ascii_lowercase())
}

fn extract_gitronimo_app(zip: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dest_dir)
        .map_err(|_| "Could not create a staging folder for the update.".to_owned())?;
    let status = Command::new("ditto")
        .args(["-x", "-k"])
        .arg(zip)
        .arg(dest_dir)
        .status()
        .map_err(|_| "Could not unpack the update.".to_owned())?;
    if !status.success() {
        return Err("Could not unpack the update.".into());
    }
    let app = dest_dir.join("GitRonimo.app");
    if !app.is_dir() {
        return Err("Could not install the update: the zip did not contain GitRonimo.app.".into());
    }
    let dest_canon = dest_dir
        .canonicalize()
        .map_err(|_| "Could not inspect the unpacked update.".to_owned())?;
    let app_canon = app
        .canonicalize()
        .map_err(|_| "Could not inspect the unpacked update.".to_owned())?;
    if !app_canon.starts_with(&dest_canon) {
        return Err(
            "Could not install the update: the zip tried to write outside the staging folder."
                .into(),
        );
    }
    if app_canon.file_name() != Some(OsStr::new("GitRonimo.app")) {
        return Err("Could not install the update: the zip did not contain GitRonimo.app.".into());
    }
    Ok(app_canon)
}

fn assess_gatekeeper(app: &Path) -> Result<(), String> {
    let codesign = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(app)
        .status()
        .map_err(|_| "Could not verify the downloaded app signature.".to_owned())?;
    if !codesign.success() {
        return Err("Could not install the update: the downloaded app is not signed.".into());
    }
    let spctl = Command::new("spctl")
        .args(["--assess", "--type", "execute"])
        .arg(app)
        .status()
        .map_err(|_| "Could not run Gatekeeper assessment.".to_owned())?;
    if spctl.success() {
        Ok(())
    } else {
        Err("Could not install the update: Gatekeeper rejected the downloaded app.".into())
    }
}

fn replace_application_bundle(staged_app: &Path, current_bundle: &Path) -> Result<(), String> {
    let parent = current_bundle
        .parent()
        .ok_or_else(|| "Could not locate the current GitRonimo.app.".to_owned())?;
    if current_bundle.file_name() != Some(OsStr::new("GitRonimo.app")) {
        return Err(
            "Could not install the update: in-app updates only replace GitRonimo.app.".into(),
        );
    }
    let backup = parent.join("GitRonimo.app.gitronimo-previous");
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|_| "Could not clear the previous update backup.".to_owned())?;
    }
    fs::rename(current_bundle, &backup)
        .map_err(|_| "Could not move the current app aside.".to_owned())?;
    let copied = Command::new("ditto")
        .arg(staged_app)
        .arg(current_bundle)
        .status();
    match copied {
        Ok(status) if status.success() => {
            let _ = fs::remove_dir_all(&backup);
            Ok(())
        }
        _ => {
            let _ = fs::rename(&backup, current_bundle);
            Err("Could not install the new GitRonimo.app.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingAppUpdate, bundle_from_executable, file_sha256_hex, install_release};
    use std::{fs, path::Path};

    #[test]
    fn cargo_run_executable_is_not_a_bundle() {
        assert!(
            bundle_from_executable(Path::new("/Users/me/gitronimo/target/debug/GitRonimo"))
                .is_none()
        );
    }

    #[test]
    fn gitronimo_app_macos_executable_is_a_bundle() {
        let path = Path::new("/Applications/GitRonimo.app/Contents/MacOS/GitRonimo");
        assert_eq!(
            bundle_from_executable(path).as_deref(),
            Some(Path::new("/Applications/GitRonimo.app"))
        );
    }

    #[test]
    fn install_refuses_unsafe_zip_name_even_from_a_bundle_path() {
        let pending = PendingAppUpdate {
            version: "1.0.1".into(),
            zip_name: "../evil.zip".into(),
            zip_url:
                "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/GitRonimo-v1.0.1.zip"
                    .into(),
            sums_url: "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/SHA256SUMS.txt"
                .into(),
        };
        let exe = Path::new("/Applications/GitRonimo.app/Contents/MacOS/GitRonimo");
        let error = install_release(&pending, exe).expect_err("refuse");
        assert!(error.contains("zip name"));
    }

    #[test]
    fn install_refuses_non_bundle_executable() {
        let pending = PendingAppUpdate {
            version: "1.0.1".into(),
            zip_name: "GitRonimo-v1.0.1.zip".into(),
            zip_url:
                "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/GitRonimo-v1.0.1.zip"
                    .into(),
            sums_url: "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/SHA256SUMS.txt"
                .into(),
        };
        let error = install_release(&pending, Path::new("/tmp/GitRonimo")).expect_err("refuse");
        assert!(error.contains("GitRonimo.app bundle"));
    }

    #[test]
    fn shasum_hex_is_64_lowercase_digits() {
        let path = std::env::temp_dir().join(format!(
            "gitronimo-shasum-{}-update-test",
            std::process::id()
        ));
        fs::write(&path, b"gitronimo-hash-fixture\n").expect("write fixture");
        let hex = file_sha256_hex(&path).expect("shasum");
        let _ = fs::remove_file(&path);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(hex, hex.to_ascii_lowercase());
    }
}
