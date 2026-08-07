//! Application use cases and ports. UI and infrastructure depend on this crate, never vice versa.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use git_domain::{RepositoryLocation, WorktreeRepository};
use serde::{Deserialize, Serialize};

const STORE_SCHEMA_VERSION: u32 = 1;
const MAXIMUM_RECENT_REPOSITORIES: usize = 12;

/// The infrastructure boundary for classifying a user-selected path.
pub trait RepositoryDiscoverer {
    /// Discovers Git's canonical repository location for `path`.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when `path` cannot be opened as a repository.
    fn discover_repository(&self, path: &Path) -> Result<RepositoryLocation, RepositoryOpenError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryOpenError {
    NotDirectory(PathBuf),
    NotRepository(PathBuf),
    BareRepository(PathBuf),
    DiscoveryFailed,
}

impl fmt::Display for RepositoryOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory(_) => formatter.write_str("Choose a folder to open a repository."),
            Self::NotRepository(_) => formatter.write_str(
                "This folder is not a Git repository. Choose its repository folder or a folder inside it.",
            ),
            Self::BareRepository(_) => formatter.write_str(
                "Bare repositories cannot be opened yet. Choose a working-tree repository instead.",
            ),
            Self::DiscoveryFailed => formatter.write_str(
                "Git could not inspect this folder. Check that Git is installed and try again.",
            ),
        }
    }
}

impl std::error::Error for RepositoryOpenError {}

/// Converts a discovered working tree into the model used by the application shell.
///
/// # Errors
///
/// Returns the discovery error, including the explicit unsupported-bare state.
pub fn open_repository(
    discoverer: &impl RepositoryDiscoverer,
    path: &Path,
) -> Result<WorktreeRepository, RepositoryOpenError> {
    match discoverer.discover_repository(path)? {
        RepositoryLocation::Worktree(repository) => Ok(repository),
        RepositoryLocation::Bare { git_dir } => Err(RepositoryOpenError::BareRepository(git_dir)),
    }
}

/// A small, versioned store for repository recents.
#[derive(Clone, Debug)]
pub struct RecentRepositoryStore {
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RecentRepositoryStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Loads recents without creating a file on a first launch.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable, malformed, or newer-version stores.
    pub fn load(&self) -> Result<Vec<PathBuf>, RecentRepositoryStoreError> {
        Ok(self.load_document()?.recent_repositories)
    }

    /// Moves `path` to the front of recents and persists the updated document.
    ///
    /// # Errors
    ///
    /// Does not overwrite a newer or malformed document.
    pub fn record(&self, path: PathBuf) -> Result<Vec<PathBuf>, RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        let mut recents = std::mem::take(&mut document.recent_repositories);
        recents.retain(|recent| recent != &path);
        recents.insert(0, path);
        recents.truncate(MAXIMUM_RECENT_REPOSITORIES);
        document.recent_repositories.clone_from(&recents);
        self.save(&document)?;
        Ok(recents)
    }

    /// Returns the last saved window geometry, if any.
    ///
    /// # Errors
    ///
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_window_geometry(
        &self,
    ) -> Result<Option<WindowGeometry>, RecentRepositoryStoreError> {
        Ok(self.load_document()?.window_geometry)
    }

    /// Stores a window geometry while retaining existing recents.
    ///
    /// # Errors
    ///
    /// Does not overwrite a newer or malformed document.
    pub fn save_window_geometry(
        &self,
        geometry: WindowGeometry,
    ) -> Result<(), RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        document.window_geometry = Some(geometry);
        self.save(&document)
    }

    /// Returns persisted ref-browser group keys.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_expanded_ref_groups(&self) -> Result<Vec<String>, RecentRepositoryStoreError> {
        Ok(self.load_document()?.expanded_ref_groups)
    }

    /// Stores ref-browser group keys while retaining recents and geometry.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_expanded_ref_groups(
        &self,
        groups: Vec<String>,
    ) -> Result<(), RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        document.expanded_ref_groups = groups;
        self.save(&document)
    }

    fn load_document(&self) -> Result<RecentRepositoryDocument, RecentRepositoryStoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let document = serde_json::from_slice::<RecentRepositoryDocument>(&bytes)
                    .map_err(RecentRepositoryStoreError::InvalidJson)?;
                if document.schema_version != STORE_SCHEMA_VERSION {
                    return Err(RecentRepositoryStoreError::UnsupportedSchema(
                        document.schema_version,
                    ));
                }
                Ok(document)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(RecentRepositoryDocument::default())
            }
            Err(error) => Err(RecentRepositoryStoreError::Io(error)),
        }
    }

    fn save(&self, document: &RecentRepositoryDocument) -> Result<(), RecentRepositoryStoreError> {
        let parent = self
            .path
            .parent()
            .ok_or(RecentRepositoryStoreError::MissingParent)?;
        fs::create_dir_all(parent).map_err(RecentRepositoryStoreError::Io)?;
        let bytes =
            serde_json::to_vec_pretty(document).map_err(RecentRepositoryStoreError::InvalidJson)?;
        let temporary_path = self.path.with_extension("tmp");
        fs::write(&temporary_path, bytes).map_err(RecentRepositoryStoreError::Io)?;
        fs::rename(temporary_path, &self.path).map_err(RecentRepositoryStoreError::Io)
    }
}

#[derive(Debug)]
pub enum RecentRepositoryStoreError {
    Io(io::Error),
    InvalidJson(serde_json::Error),
    MissingParent,
    UnsupportedSchema(u32),
}

impl fmt::Display for RecentRepositoryStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("Gitronimo could not read its recent repositories."),
            Self::InvalidJson(_) => formatter.write_str("Gitronimo's recent repository data is invalid."),
            Self::MissingParent => formatter.write_str("Gitronimo's recent repository location is invalid."),
            Self::UnsupportedSchema(_) => formatter.write_str(
                "Gitronimo's recent repository data was created by a newer version and was left unchanged.",
            ),
        }
    }
}

impl std::error::Error for RecentRepositoryStoreError {}

#[derive(Deserialize, Serialize)]
struct RecentRepositoryDocument {
    schema_version: u32,
    recent_repositories: Vec<PathBuf>,
    #[serde(default)]
    window_geometry: Option<WindowGeometry>,
    #[serde(default)]
    expanded_ref_groups: Vec<String>,
}

impl Default for RecentRepositoryDocument {
    fn default() -> Self {
        Self {
            schema_version: STORE_SCHEMA_VERSION,
            recent_repositories: Vec::new(),
            window_geometry: None,
            expanded_ref_groups: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{RecentRepositoryStore, RecentRepositoryStoreError, WindowGeometry};

    static NEXT_STORE: AtomicUsize = AtomicUsize::new(0);

    fn temporary_store() -> (std::path::PathBuf, RecentRepositoryStore) {
        let directory = std::env::temp_dir().join(format!(
            "gitronimo-app-core-{}-{}",
            std::process::id(),
            NEXT_STORE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join("recents.json");
        (directory, RecentRepositoryStore::new(path))
    }

    #[test]
    fn recents_are_deduplicated_and_survive_restart() {
        let (directory, store) = temporary_store();
        let first = directory.join("first");
        let second = directory.join("second");

        store
            .record(first.clone())
            .expect("first record should save");
        store
            .record(second.clone())
            .expect("second record should save");
        let recents = store
            .record(first.clone())
            .expect("duplicate should move to front");

        assert_eq!(recents, vec![first, second]);
        assert_eq!(store.load().expect("store should reload"), recents);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn expanded_ref_groups_survive_restart() {
        let (directory, store) = temporary_store();
        let groups = vec!["local:feature".into(), "remote:origin/topic".into()];
        store
            .save_expanded_ref_groups(groups.clone())
            .expect("groups should save");
        assert_eq!(
            store
                .load_expanded_ref_groups()
                .expect("groups should reload"),
            groups
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn newer_schema_is_rejected_without_overwriting_it() {
        let (directory, store) = temporary_store();
        let store_path = directory.join("recents.json");
        fs::create_dir_all(&directory).expect("store directory should create");
        let newer = br#"{"schema_version":2,"recent_repositories":[]}"#;
        fs::write(&store_path, newer).expect("newer store should write");

        assert!(matches!(
            store.record(directory.join("repository")),
            Err(RecentRepositoryStoreError::UnsupportedSchema(2))
        ));
        assert_eq!(
            fs::read(&store_path).expect("newer store should remain"),
            newer
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn geometry_and_recents_share_one_schema_safe_document() {
        let (directory, store) = temporary_store();
        let repository = directory.join("repository");
        let geometry = WindowGeometry {
            x: 30.0,
            y: 40.0,
            width: 900.0,
            height: 700.0,
        };

        store
            .record(repository.clone())
            .expect("recent should save");
        store
            .save_window_geometry(geometry)
            .expect("geometry should save");

        assert_eq!(
            store.load().expect("recents should remain"),
            vec![repository]
        );
        assert_eq!(
            store.load_window_geometry().expect("geometry should load"),
            Some(geometry)
        );
        let _ = fs::remove_dir_all(directory);
    }
}
