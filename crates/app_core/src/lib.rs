//! Application use cases and ports. UI and infrastructure depend on this crate, never vice versa.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use git_domain::{
    HostedRepository, MergeMethod, PullRequestComment, PullRequestDetail, PullRequestSummary,
    RecoveryRecord, RepositoryLocation, ServiceAccount, WorktreeRepository,
};
use serde::{Deserialize, Serialize};

const STORE_SCHEMA_VERSION: u32 = 1;
const MAXIMUM_RECENT_REPOSITORIES: usize = 12;
const RECOVERY_JOURNAL_SCHEMA_VERSION: u32 = 1;
const MAXIMUM_RECOVERY_JOURNAL_ENTRIES: usize = 20;

/// The infrastructure boundary for classifying a user-selected path.
pub trait RepositoryDiscoverer {
    /// Discovers Git's canonical repository location for `path`.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when `path` cannot be opened as a repository.
    fn discover_repository(&self, path: &Path) -> Result<RepositoryLocation, RepositoryOpenError>;
}

/// Non-secret lookup key for a credential held by platform storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretKey {
    pub service: String,
    pub account: String,
}

/// Platform-owned secret storage boundary.
pub trait SecretStore {
    /// Reads a secret without exposing it in application state.
    ///
    /// # Errors
    /// Returns a platform storage failure.
    fn read(&self, key: &SecretKey) -> Result<Option<String>, SecretStoreError>;
    /// Stores a secret in the platform credential store.
    ///
    /// # Errors
    /// Returns a platform storage failure.
    fn write(&self, key: &SecretKey, value: &str) -> Result<(), SecretStoreError>;
    /// Deletes a secret from the platform credential store.
    ///
    /// # Errors
    /// Returns a platform storage failure.
    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretStoreError {
    Unavailable,
    CommandFailed,
}

/// Provider-neutral hosting API port. Tokens exist only during calls and are
/// never part of the returned application models.
pub trait HostingService {
    /// Validates a provider token and returns non-secret account metadata.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn authenticate(&self, token: &str) -> Result<ServiceAccount, HostingError>;
    /// Lists repositories visible to the authenticated account.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn repositories(&self, token: &str) -> Result<Vec<HostedRepository>, HostingError>;
    /// Lists open pull requests for one hosted repository.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn pull_requests(
        &self,
        token: &str,
        repository: &HostedRepository,
    ) -> Result<Vec<PullRequestSummary>, HostingError>;
    /// Loads one pull request's description, files, and comments.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn pull_request_detail(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
    ) -> Result<PullRequestDetail, HostingError>;
    /// Creates a pull request from `head` into `base`.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn create_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequestSummary, HostingError>;
    /// Adds a comment to a pull request.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn comment_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
        body: &str,
    ) -> Result<PullRequestComment, HostingError>;
    /// Merges a pull request using an explicit method.
    ///
    /// # Errors
    /// Returns authentication, rate-limit, transport, API, or parse failures.
    fn merge_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
        method: MergeMethod,
    ) -> Result<(), HostingError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostingError {
    Authentication,
    RateLimited { retry_after_seconds: Option<u64> },
    Network,
    Api(String),
    Parse,
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

    /// Moves malformed preferences aside and restores an empty versioned document.
    ///
    /// # Errors
    /// Returns an error for unreadable preferences, an unavailable backup location, or a newer schema.
    pub fn recover_corrupted_preferences(&self) -> Result<bool, RecentRepositoryStoreError> {
        match self.load_document() {
            Ok(_) => Ok(false),
            Err(RecentRepositoryStoreError::InvalidJson(_)) => {
                let mut backup = self.path.with_extension("corrupt");
                let mut attempt = 1_u32;
                while backup.exists() {
                    backup = self.path.with_extension(format!("corrupt-{attempt}"));
                    attempt = attempt.saturating_add(1);
                }
                fs::rename(&self.path, backup).map_err(RecentRepositoryStoreError::Io)?;
                self.save(&RecentRepositoryDocument::default())?;
                Ok(true)
            }
            Err(error) => Err(error),
        }
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

    /// Removes one path from recents and persists the updated document.
    ///
    /// # Errors
    ///
    /// Does not overwrite a newer or malformed document.
    pub fn remove(&self, path: &Path) -> Result<Vec<PathBuf>, RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        document.recent_repositories.retain(|recent| recent != path);
        let recents = document.recent_repositories.clone();
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

    /// Returns the last saved sidebar width, if any.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_sidebar_width(&self) -> Result<Option<f32>, RecentRepositoryStoreError> {
        Ok(self.load_document()?.sidebar_width)
    }

    /// Persists the sidebar width while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_sidebar_width(&self, width: f32) -> Result<(), RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        document.sidebar_width = Some(width);
        self.save(&document)
    }

    /// Returns the last saved working-copy list pane width, if any.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_list_pane_width(&self) -> Result<Option<f32>, RecentRepositoryStoreError> {
        Ok(self.load_document()?.list_pane_width)
    }

    /// Persists the list pane width while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_list_pane_width(&self, width: f32) -> Result<(), RecentRepositoryStoreError> {
        let mut document = self.load_document()?;
        document.list_pane_width = Some(width);
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

/// A versioned, bounded journal of pre-operation refs recorded before each
/// history-changing Git operation. Kept without credentials; corrupt data is
/// quarantined by the same policy as preferences.
#[derive(Clone, Debug)]
pub struct RecoveryJournalStore {
    path: PathBuf,
}

/// One journaled recovery snapshot for a repository.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecoveryJournalEntry {
    pub repository_path: PathBuf,
    pub recorded_at_millis: u64,
    pub record: RecoveryRecord,
}

impl RecoveryJournalStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Prepends one entry and persists the bounded journal.
    ///
    /// # Errors
    /// Returns an error for an unreadable store, a newer schema, or a failed write.
    pub fn record_entry(
        &self,
        repository_path: PathBuf,
        record: RecoveryRecord,
    ) -> Result<(), RecoveryJournalStoreError> {
        let mut document = self.load_document()?;
        let mut entries = std::mem::take(&mut document.entries);
        let recorded_at_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            });
        entries.insert(
            0,
            RecoveryJournalEntry {
                repository_path,
                recorded_at_millis,
                record,
            },
        );
        entries.truncate(MAXIMUM_RECOVERY_JOURNAL_ENTRIES);
        document.entries.clone_from(&entries);
        self.save(&document)
    }

    /// Returns the journal entries, newest first.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`RecentRepositoryStore::load`].
    pub fn load(&self) -> Result<Vec<RecoveryJournalEntry>, RecoveryJournalStoreError> {
        Ok(self.load_document()?.entries)
    }

    fn load_document(&self) -> Result<RecoveryJournalDocument, RecoveryJournalStoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let document = serde_json::from_slice::<RecoveryJournalDocument>(&bytes)
                    .map_err(RecoveryJournalStoreError::InvalidJson)?;
                if document.schema_version != RECOVERY_JOURNAL_SCHEMA_VERSION {
                    return Err(RecoveryJournalStoreError::UnsupportedSchema(
                        document.schema_version,
                    ));
                }
                Ok(document)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(RecoveryJournalDocument::default())
            }
            Err(error) => Err(RecoveryJournalStoreError::Io(error)),
        }
    }

    fn save(&self, document: &RecoveryJournalDocument) -> Result<(), RecoveryJournalStoreError> {
        let parent = self
            .path
            .parent()
            .ok_or(RecoveryJournalStoreError::MissingParent)?;
        fs::create_dir_all(parent).map_err(RecoveryJournalStoreError::Io)?;
        let bytes =
            serde_json::to_vec_pretty(document).map_err(RecoveryJournalStoreError::InvalidJson)?;
        let temporary_path = self.path.with_extension("tmp");
        fs::write(&temporary_path, bytes).map_err(RecoveryJournalStoreError::Io)?;
        fs::rename(temporary_path, &self.path).map_err(RecoveryJournalStoreError::Io)
    }
}

#[derive(Debug)]
pub enum RecoveryJournalStoreError {
    Io(io::Error),
    InvalidJson(serde_json::Error),
    MissingParent,
    UnsupportedSchema(u32),
}

impl fmt::Display for RecoveryJournalStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => {
                formatter.write_str("Gitronimo could not read its operation recovery journal.")
            }
            Self::InvalidJson(_) => {
                formatter.write_str("Gitronimo's operation recovery journal is invalid.")
            }
            Self::MissingParent => {
                formatter.write_str("Gitronimo's recovery journal location is invalid.")
            }
            Self::UnsupportedSchema(_) => formatter.write_str(
                "Gitronimo's recovery journal was created by a newer version and was left unchanged.",
            ),
        }
    }
}

impl std::error::Error for RecoveryJournalStoreError {}

#[derive(Deserialize, Serialize)]
struct RecentRepositoryDocument {
    schema_version: u32,
    recent_repositories: Vec<PathBuf>,
    #[serde(default)]
    window_geometry: Option<WindowGeometry>,
    #[serde(default)]
    expanded_ref_groups: Vec<String>,
    #[serde(default)]
    sidebar_width: Option<f32>,
    #[serde(default)]
    list_pane_width: Option<f32>,
}

impl Default for RecentRepositoryDocument {
    fn default() -> Self {
        Self {
            schema_version: STORE_SCHEMA_VERSION,
            recent_repositories: Vec::new(),
            window_geometry: None,
            expanded_ref_groups: Vec::new(),
            sidebar_width: None,
            list_pane_width: None,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct RecoveryJournalDocument {
    schema_version: u32,
    #[serde(default)]
    entries: Vec<RecoveryJournalEntry>,
}

impl Default for RecoveryJournalDocument {
    fn default() -> Self {
        Self {
            schema_version: RECOVERY_JOURNAL_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{
        RecentRepositoryStore, RecentRepositoryStoreError, RecoveryJournalStore, WindowGeometry,
    };
    use git_domain::{GitPath, RecoveredBranchTip, RecoveryRecord};

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
    fn remove_drops_one_recent_and_persists() {
        let (directory, store) = temporary_store();
        let first = directory.join("first");
        let second = directory.join("second");
        store
            .record(first.clone())
            .expect("first record should save");
        store
            .record(second.clone())
            .expect("second record should save");

        let recents = store.remove(&first).expect("remove should save");
        assert_eq!(recents, vec![second.clone()]);
        assert_eq!(store.load().expect("store should reload"), recents);
        let _ = fs::remove_dir_all(directory);
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
    fn pane_widths_survive_restart() {
        let (directory, store) = temporary_store();
        store
            .save_sidebar_width(248.0)
            .expect("sidebar width should save");
        store
            .save_list_pane_width(360.0)
            .expect("list pane width should save");
        assert_eq!(
            store
                .load_sidebar_width()
                .expect("sidebar width should reload"),
            Some(248.0)
        );
        assert_eq!(
            store
                .load_list_pane_width()
                .expect("list pane width should reload"),
            Some(360.0)
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn malformed_preferences_are_quarantined_and_recreated() {
        let (directory, store) = temporary_store();
        let store_path = directory.join("recents.json");
        fs::create_dir_all(&directory).expect("store directory should create");
        fs::write(&store_path, b"not valid json").expect("malformed preferences should write");

        assert!(
            store
                .recover_corrupted_preferences()
                .expect("malformed preferences should recover")
        );
        assert_eq!(
            fs::read(directory.join("recents.corrupt")).expect("backup should remain"),
            b"not valid json"
        );
        assert!(
            store
                .load()
                .expect("fresh preferences should load")
                .is_empty()
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

    fn temporary_journal() -> (std::path::PathBuf, RecoveryJournalStore) {
        let directory = std::env::temp_dir().join(format!(
            "gitronimo-journal-{}-{}",
            std::process::id(),
            NEXT_STORE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join("recovery-journal.json");
        (directory, RecoveryJournalStore::new(path))
    }

    fn recovery_record(prefix: &str) -> RecoveryRecord {
        RecoveryRecord {
            old_head: Some(format!("{prefix}head").into_bytes()),
            head_name: Some(GitPath(format!("{prefix}name").into_bytes())),
            branch_tips: vec![RecoveredBranchTip {
                name: GitPath(format!("{prefix}ref").into_bytes()),
                oid: format!("{prefix}oid").into_bytes(),
            }],
        }
    }

    #[test]
    fn recovery_journal_persists_entries_newest_first() {
        let (directory, store) = temporary_journal();
        let repository = directory.join("repository");

        store
            .record_entry(repository.clone(), recovery_record("one-"))
            .expect("first entry should save");
        store
            .record_entry(repository.clone(), recovery_record("two-"))
            .expect("second entry should save");

        let entries = store.load().expect("journal should reload");
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].record.old_head.as_deref(),
            Some(b"two-head".as_slice())
        );
        assert_eq!(
            entries[0].repository_path, repository,
            "entries should retain their repository"
        );
        assert_eq!(
            entries[1].record.old_head.as_deref(),
            Some(b"one-head".as_slice())
        );
        assert!(
            entries[0].recorded_at_millis >= entries[1].recorded_at_millis,
            "newest entry should be first"
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn recovery_journal_is_bounded_to_twenty_entries() {
        let (directory, store) = temporary_journal();
        let repository = directory.join("repository");

        for index in 0..25 {
            store
                .record_entry(repository.clone(), recovery_record(&format!("{index}-")))
                .expect("entry should save");
        }

        let entries = store.load().expect("journal should reload");
        assert_eq!(entries.len(), 20);
        assert_eq!(
            entries.first().unwrap().record.old_head.as_deref(),
            Some(b"24-head".as_slice()),
            "newest entries should be retained"
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn newer_recovery_journal_schema_is_rejected_without_overwriting() {
        let (directory, store) = temporary_journal();
        let store_path = directory.join("recovery-journal.json");
        fs::create_dir_all(&directory).expect("store directory should create");
        let newer = br#"{"schema_version":2,"entries":[]}"#;
        fs::write(&store_path, newer).expect("newer journal should write");

        assert!(matches!(
            store.record_entry(directory.join("repository"), recovery_record("x-")),
            Err(super::RecoveryJournalStoreError::UnsupportedSchema(2))
        ));
        assert_eq!(
            fs::read(&store_path).expect("newer journal should remain"),
            newer
        );
        let _ = fs::remove_dir_all(directory);
    }
}
