//! Application use cases and ports. UI and infrastructure depend on this crate, never vice versa.

use std::{
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use git_domain::{
    HostedRepository, MergeMethod, PullRequestComment, PullRequestDetail, PullRequestSummary,
    RecoveryRecord, RepositoryLocation, ServiceAccount, WorktreeRepository,
};
use serde::{Deserialize, Serialize};

mod git_engine;
mod workflow;

pub use git_engine::{
    EngineQuery, GitBackendError, GitEngineKind, GitHistoryQuery, GitIndexMutate, GitNetwork,
    GitObjectQuery, GitRefQuery, query_preferring,
};
pub use workflow::{
    RepositoryWorkflow, WorkflowBaseBranch, WorkflowGitStep, WorkflowKind, WorkflowTopicType,
};

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
    /// Serializes load-modify-save so concurrent preference writers cannot wipe
    /// each other's fields (e.g. window geometry racing branch pins).
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

fn preferences_lock_for(path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

impl RecentRepositoryStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        let lock = preferences_lock_for(&path);
        Self { path, lock }
    }

    /// Moves malformed preferences aside and restores an empty versioned document.
    ///
    /// # Errors
    /// Returns an error for unreadable preferences, an unavailable backup location, or a newer schema.
    pub fn recover_corrupted_preferences(&self) -> Result<bool, RecentRepositoryStoreError> {
        let _guard = self.lock_preferences();
        match Self::read_document(&self.path) {
            Ok(_) => Ok(false),
            Err(RecentRepositoryStoreError::InvalidJson(_)) => {
                let mut backup = self.path.with_extension("corrupt");
                let mut attempt = 1_u32;
                while backup.exists() {
                    backup = self.path.with_extension(format!("corrupt-{attempt}"));
                    attempt = attempt.saturating_add(1);
                }
                fs::rename(&self.path, backup).map_err(RecentRepositoryStoreError::Io)?;
                Self::write_document(&self.path, &RecentRepositoryDocument::default())?;
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
        self.with_document(|document| document.recent_repositories.clone())
    }

    /// Moves `path` to the front of recents and persists the updated document.
    ///
    /// # Errors
    ///
    /// Does not overwrite a newer or malformed document.
    pub fn record(&self, path: PathBuf) -> Result<Vec<PathBuf>, RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            let mut recents = std::mem::take(&mut document.recent_repositories);
            recents.retain(|recent| recent != &path);
            recents.insert(0, path);
            recents.truncate(MAXIMUM_RECENT_REPOSITORIES);
            document.recent_repositories.clone_from(&recents);
            recents
        })
    }

    /// Removes one path from recents and persists the updated document.
    ///
    /// # Errors
    ///
    /// Does not overwrite a newer or malformed document.
    pub fn remove(&self, path: &Path) -> Result<Vec<PathBuf>, RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.recent_repositories.retain(|recent| recent != path);
            document
                .repository_folders
                .remove(&path.to_string_lossy().into_owned());
            document.recent_repositories.clone()
        })
    }

    /// Returns the last saved window geometry, if any.
    ///
    /// # Errors
    ///
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_window_geometry(
        &self,
    ) -> Result<Option<WindowGeometry>, RecentRepositoryStoreError> {
        self.with_document(|document| document.window_geometry)
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
        self.with_mut_document(|document| {
            document.window_geometry = Some(geometry);
        })
    }

    /// Returns persisted ref-browser group keys.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_expanded_ref_groups(&self) -> Result<Vec<String>, RecentRepositoryStoreError> {
        self.with_document(|document| document.expanded_ref_groups.clone())
    }

    /// Stores ref-browser group keys while retaining recents and geometry.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_expanded_ref_groups(
        &self,
        groups: Vec<String>,
    ) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.expanded_ref_groups = groups;
        })
    }

    /// Returns the last saved sidebar width, if any.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_sidebar_width(&self) -> Result<Option<f32>, RecentRepositoryStoreError> {
        self.with_document(|document| document.sidebar_width)
    }

    /// Persists the sidebar width while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_sidebar_width(&self, width: f32) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.sidebar_width = Some(width);
        })
    }

    /// Returns the last saved working-copy list pane width, if any.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_list_pane_width(&self) -> Result<Option<f32>, RecentRepositoryStoreError> {
        self.with_document(|document| document.list_pane_width)
    }

    /// Persists the list pane width while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_list_pane_width(&self, width: f32) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.list_pane_width = Some(width);
        })
    }

    /// Loads bookmark folders and repository→folder membership.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_bookmark_organization(
        &self,
    ) -> Result<BookmarkOrganization, RecentRepositoryStoreError> {
        self.with_document(|document| BookmarkOrganization {
            folders: document.bookmark_folders.clone(),
            repository_folders: document.repository_folders.clone(),
        })
    }

    /// Persists bookmark folders and membership while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_bookmark_organization(
        &self,
        organization: &BookmarkOrganization,
    ) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.bookmark_folders.clone_from(&organization.folders);
            document
                .repository_folders
                .clone_from(&organization.repository_folders);
        })
    }

    /// Loads pinned and archived branch names for one repository.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_branch_organization(
        &self,
        repository: &Path,
    ) -> Result<BranchOrganization, RecentRepositoryStoreError> {
        let key = repository.to_string_lossy().into_owned();
        self.with_document(|document| {
            document
                .branch_organization
                .get(&key)
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Persists pinned and archived branch names for one repository.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_branch_organization(
        &self,
        repository: &Path,
        organization: &BranchOrganization,
    ) -> Result<(), RecentRepositoryStoreError> {
        let key = repository.to_string_lossy().into_owned();
        self.with_mut_document(|document| {
            if organization.pinned.is_empty() && organization.archived.is_empty() {
                document.branch_organization.remove(&key);
            } else {
                document
                    .branch_organization
                    .insert(key, organization.clone());
            }
        })
    }

    /// Loads the branching convention for one repository.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_workflow(
        &self,
        repository: &Path,
    ) -> Result<Option<crate::RepositoryWorkflow>, RecentRepositoryStoreError> {
        let key = repository.to_string_lossy().into_owned();
        self.with_document(|document| document.workflows.get(&key).cloned())
    }

    /// Whether Settings forces the installed Git executable instead of `gix`.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_use_system_git(&self) -> Result<bool, RecentRepositoryStoreError> {
        self.with_document(|document| document.use_system_git)
    }

    /// Persists the system-Git override while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_use_system_git(
        &self,
        use_system_git: bool,
    ) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.use_system_git = use_system_git;
        })
    }

    /// Whether Settings stashes dirty work before switch and pull.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_auto_stash(&self) -> Result<bool, RecentRepositoryStoreError> {
        self.with_document(|document| document.auto_stash)
    }

    /// Persists auto-stash while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_auto_stash(&self, auto_stash: bool) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.auto_stash = auto_stash;
        })
    }

    /// Whether Settings may check GitHub Releases and replace this `.app`.
    ///
    /// # Errors
    /// Returns the same schema and read errors as [`Self::load`].
    pub fn load_in_app_updates(&self) -> Result<bool, RecentRepositoryStoreError> {
        self.with_document(|document| document.in_app_updates)
    }

    /// Persists the in-app updates toggle while retaining other preferences.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_in_app_updates(
        &self,
        in_app_updates: bool,
    ) -> Result<(), RecentRepositoryStoreError> {
        self.with_mut_document(|document| {
            document.in_app_updates = in_app_updates;
        })
    }

    /// Persists the branching convention for one repository. `None` clears it.
    ///
    /// # Errors
    /// Does not overwrite a newer or malformed document.
    pub fn save_workflow(
        &self,
        repository: &Path,
        workflow: Option<&crate::RepositoryWorkflow>,
    ) -> Result<(), RecentRepositoryStoreError> {
        let key = repository.to_string_lossy().into_owned();
        self.with_mut_document(|document| match workflow {
            Some(workflow) => {
                document.workflows.insert(key, workflow.clone());
            }
            None => {
                document.workflows.remove(&key);
            }
        })
    }

    fn lock_preferences(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with_document<R>(
        &self,
        f: impl FnOnce(&RecentRepositoryDocument) -> R,
    ) -> Result<R, RecentRepositoryStoreError> {
        let _guard = self.lock_preferences();
        let document = Self::read_document(&self.path)?;
        Ok(f(&document))
    }

    fn with_mut_document<R>(
        &self,
        f: impl FnOnce(&mut RecentRepositoryDocument) -> R,
    ) -> Result<R, RecentRepositoryStoreError> {
        let _guard = self.lock_preferences();
        let mut document = Self::read_document(&self.path)?;
        let result = f(&mut document);
        Self::write_document(&self.path, &document)?;
        Ok(result)
    }

    fn read_document(path: &Path) -> Result<RecentRepositoryDocument, RecentRepositoryStoreError> {
        match fs::read(path) {
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

    fn write_document(
        path: &Path,
        document: &RecentRepositoryDocument,
    ) -> Result<(), RecentRepositoryStoreError> {
        let parent = path
            .parent()
            .ok_or(RecentRepositoryStoreError::MissingParent)?;
        fs::create_dir_all(parent).map_err(RecentRepositoryStoreError::Io)?;
        let bytes =
            serde_json::to_vec_pretty(document).map_err(RecentRepositoryStoreError::InvalidJson)?;
        let temporary_path = path.with_extension("tmp");
        fs::write(&temporary_path, bytes).map_err(RecentRepositoryStoreError::Io)?;
        fs::rename(temporary_path, path).map_err(RecentRepositoryStoreError::Io)
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BookmarkFolder {
    pub id: String,
    pub name: String,
    #[serde(default = "default_folder_expanded")]
    pub expanded: bool,
}

fn default_folder_expanded() -> bool {
    true
}

/// Per-repository branch presentation flags. Pinned branches sort first in the
/// sidebar; archived branches move to their own section.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct BranchOrganization {
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub archived: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct BookmarkOrganization {
    pub folders: Vec<BookmarkFolder>,
    /// Absolute repository path → folder id. Missing key means root.
    pub repository_folders: std::collections::BTreeMap<String, String>,
}

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
    #[serde(default)]
    bookmark_folders: Vec<BookmarkFolder>,
    #[serde(default)]
    repository_folders: std::collections::BTreeMap<String, String>,
    /// Absolute repository path → pinned/archived branch names.
    #[serde(default)]
    branch_organization: std::collections::BTreeMap<String, BranchOrganization>,
    /// Absolute repository path → branching convention.
    #[serde(default)]
    workflows: std::collections::BTreeMap<String, crate::RepositoryWorkflow>,
    /// When true, skip `gix` and use the installed Git executable.
    #[serde(default)]
    use_system_git: bool,
    /// When true, stash dirty work before switch and pull, then reapply it.
    #[serde(default)]
    auto_stash: bool,
    /// When true, Settings can check GitHub Releases and install a notarized zip.
    #[serde(default)]
    in_app_updates: bool,
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
            bookmark_folders: Vec::new(),
            repository_folders: std::collections::BTreeMap::new(),
            branch_organization: std::collections::BTreeMap::new(),
            workflows: std::collections::BTreeMap::new(),
            use_system_git: false,
            auto_stash: false,
            in_app_updates: false,
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
        BranchOrganization, RecentRepositoryStore, RecentRepositoryStoreError,
        RecoveryJournalStore, WindowGeometry,
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
    fn branch_organization_is_scoped_per_repository() {
        let (directory, store) = temporary_store();
        let first = directory.join("first");
        let second = directory.join("second");
        let organization = BranchOrganization {
            pinned: vec!["main".into()],
            archived: vec!["old".into()],
        };
        store
            .save_branch_organization(&first, &organization)
            .expect("save should persist");
        assert_eq!(
            store.load_branch_organization(&first).expect("load first"),
            organization
        );
        assert_eq!(
            store
                .load_branch_organization(&second)
                .expect("empty second"),
            BranchOrganization::default()
        );
        store
            .save_branch_organization(&first, &BranchOrganization::default())
            .expect("empty org should clear");
        assert_eq!(
            store
                .load_branch_organization(&first)
                .expect("cleared first"),
            BranchOrganization::default()
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn workflow_is_scoped_per_repository() {
        let (directory, store) = temporary_store();
        let first = directory.join("first");
        let second = directory.join("second");
        let workflow = crate::RepositoryWorkflow::github_flow("main");
        store.save_workflow(&first, Some(&workflow)).expect("save");
        assert_eq!(
            store.load_workflow(&first).expect("load first"),
            Some(workflow)
        );
        assert_eq!(store.load_workflow(&second).expect("empty second"), None);
        store.save_workflow(&first, None).expect("clear");
        assert_eq!(store.load_workflow(&first).expect("cleared"), None);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn concurrent_preference_writes_keep_branch_pins() {
        use std::thread;

        let (directory, store) = temporary_store();
        let prefs_path = directory.join("recents.json");
        // Distinct `new` instances must still share a path lock (app code constructs
        // fresh stores for open/record while the window keeps another handle).
        let geometry_store = RecentRepositoryStore::new(prefs_path.clone());
        let pin_store = RecentRepositoryStore::new(prefs_path);
        let repository = directory.join("repo");
        let organization = BranchOrganization {
            pinned: vec!["feature/ui-improvements".into(), "main".into()],
            archived: Vec::new(),
        };
        pin_store
            .save_branch_organization(&repository, &organization)
            .expect("initial pin save");

        let repo_for_pins = repository.clone();
        let geometry = thread::spawn(move || {
            for index in 0..40 {
                geometry_store
                    .save_window_geometry(WindowGeometry {
                        x: f32::from(u16::try_from(index).unwrap_or(u16::MAX)),
                        y: 0.0,
                        width: 1200.0,
                        height: 800.0,
                    })
                    .expect("geometry save");
            }
        });
        let pins = thread::spawn(move || {
            for index in 0..40 {
                let mut next = organization.clone();
                if index % 2 == 0 {
                    next.pinned.push(format!("extra-{index}"));
                }
                pin_store
                    .save_branch_organization(&repo_for_pins, &next)
                    .expect("pin save");
            }
        });
        geometry.join().expect("geometry thread");
        pins.join().expect("pin thread");

        let loaded = store
            .load_branch_organization(&repository)
            .expect("pins should survive geometry races");
        assert!(
            loaded.pinned.contains(&"feature/ui-improvements".into()),
            "expected pinned branch to survive concurrent geometry writes: {loaded:?}"
        );
        assert!(store.load_window_geometry().expect("geometry").is_some());
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
    fn bookmark_folders_survive_restart() {
        use super::{BookmarkFolder, BookmarkOrganization};

        let (directory, store) = temporary_store();
        let repo = directory.join("repo");
        store.record(repo.clone()).expect("record repo");
        let organization = BookmarkOrganization {
            folders: vec![BookmarkFolder {
                id: "folder-1".into(),
                name: "work".into(),
                expanded: true,
            }],
            repository_folders: std::collections::BTreeMap::from([(
                repo.to_string_lossy().into_owned(),
                "folder-1".into(),
            )]),
        };
        store
            .save_bookmark_organization(&organization)
            .expect("organization should save");
        assert_eq!(
            store
                .load_bookmark_organization()
                .expect("organization should reload"),
            organization
        );
        assert_eq!(store.load().expect("recents remain"), vec![repo]);
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

    #[test]
    fn use_system_git_defaults_off_and_persists() {
        let (directory, store) = temporary_store();
        assert!(
            !store
                .load_use_system_git()
                .expect("missing store should default off")
        );
        store
            .save_use_system_git(true)
            .expect("override should save");
        assert!(store.load_use_system_git().expect("override should load"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn auto_stash_defaults_off_and_persists() {
        let (directory, store) = temporary_store();
        assert!(
            !store
                .load_auto_stash()
                .expect("missing store should default off")
        );
        store.save_auto_stash(true).expect("override should save");
        assert!(store.load_auto_stash().expect("override should load"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn in_app_updates_defaults_off_and_persists() {
        let (directory, store) = temporary_store();
        assert!(
            !store
                .load_in_app_updates()
                .expect("missing store should default off")
        );
        store
            .save_in_app_updates(true)
            .expect("override should save");
        assert!(store.load_in_app_updates().expect("override should load"));
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
