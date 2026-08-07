//! Safe adapter for the installed Git executable.

use std::{
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, ExitStatus, Output, Stdio},
};

use app_core::{RepositoryDiscoverer, RepositoryOpenError};
use git_domain::{RepositoryLocation, WorktreeRepository};

const MACOS_GIT_PATHS: [&str; 2] = ["/opt/homebrew/bin/git", "/usr/local/bin/git"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitExecutable(PathBuf);

impl GitExecutable {
    /// Finds Git without assuming a Finder-launched process inherited a shell PATH.
    ///
    /// # Errors
    ///
    /// Returns an error when no candidate can run `git --version`.
    pub fn discover() -> io::Result<Self> {
        Self::discover_from(git_candidates())
    }

    /// Finds the first candidate that accepts `git --version`.
    ///
    /// # Errors
    ///
    /// Returns an error when no candidate can be started successfully.
    pub fn discover_from(candidates: impl IntoIterator<Item = PathBuf>) -> io::Result<Self> {
        candidates
            .into_iter()
            .find_map(|path| Self::from_path(path).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Git executable was not found"))
    }

    /// Validates one executable path.
    ///
    /// # Errors
    ///
    /// Returns an error when the candidate cannot run `git --version`.
    pub fn from_path(path: impl Into<PathBuf>) -> io::Result<Self> {
        let executable = Self(path.into());
        executable.version().map(|_| executable)
    }

    /// Returns the installed Git version.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot start or rejects `--version`.
    pub fn version(&self) -> io::Result<String> {
        let output = Command::new(&self.0).arg("--version").output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(io::Error::other("Git did not accept --version"))
        }
    }

    /// Runs Git in `directory` with individual, shell-free arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when the process cannot be started or its output captured.
    pub fn run<I, S>(&self, directory: &Path, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command(directory, args).output()
    }

    /// Classifies a selected directory using Git's canonical worktree and Git-dir paths.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when Git cannot classify the selected path.
    pub fn discover_repository(
        &self,
        path: &Path,
    ) -> Result<RepositoryLocation, RepositoryOpenError> {
        if !path.is_dir() {
            return Err(RepositoryOpenError::NotDirectory(path.to_path_buf()));
        }

        let bare = self
            .run(path, ["rev-parse", "--is-bare-repository"])
            .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
        if !bare.status.success() {
            return Err(RepositoryOpenError::NotRepository(path.to_path_buf()));
        }

        match String::from_utf8_lossy(&bare.stdout).trim() {
            "true" => path
                .canonicalize()
                .map(|git_dir| RepositoryLocation::Bare { git_dir })
                .map_err(|_| RepositoryOpenError::DiscoveryFailed),
            "false" => {
                let locations = self
                    .run(path, ["rev-parse", "--show-toplevel", "--absolute-git-dir"])
                    .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                if !locations.status.success() {
                    return Err(RepositoryOpenError::DiscoveryFailed);
                }
                let locations = String::from_utf8_lossy(&locations.stdout);
                let mut lines = locations.lines();
                let Some(worktree_root) = lines.next() else {
                    return Err(RepositoryOpenError::DiscoveryFailed);
                };
                let Some(git_dir) = lines.next() else {
                    return Err(RepositoryOpenError::DiscoveryFailed);
                };
                let worktree_root = PathBuf::from(worktree_root)
                    .canonicalize()
                    .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                let git_dir = PathBuf::from(git_dir)
                    .canonicalize()
                    .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                Ok(RepositoryLocation::Worktree(WorktreeRepository {
                    worktree_root,
                    git_dir,
                }))
            }
            _ => Err(RepositoryOpenError::DiscoveryFailed),
        }
    }

    /// Starts Git with piped standard streams for progress and cancellation.
    ///
    /// # Errors
    ///
    /// Returns an error when the process cannot be started.
    pub fn start<I, S>(&self, directory: &Path, args: I) -> io::Result<GitChild>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let child = self
            .command(directory, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(GitChild(child))
    }

    fn command<I, S>(&self, directory: &Path, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.0);
        command.current_dir(directory).args(args);
        command
    }
}

impl RepositoryDiscoverer for GitExecutable {
    fn discover_repository(&self, path: &Path) -> Result<RepositoryLocation, RepositoryOpenError> {
        self.discover_repository(path)
    }
}

#[derive(Debug)]
pub struct GitChild(Child);

impl GitChild {
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.0.stderr.take()
    }

    /// Terminates the child process.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot terminate the child.
    pub fn cancel(&mut self) -> io::Result<()> {
        self.0.kill()
    }

    /// Waits for the child process to exit.
    ///
    /// # Errors
    ///
    /// Returns an error when waiting for the child fails.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.0.wait()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorcelainStatus {
    pub headers: Vec<Vec<u8>>,
    pub entries: Vec<StatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusEntry {
    Untracked(Vec<u8>),
    Ignored(Vec<u8>),
    Tracked(Vec<u8>),
}

#[must_use]
pub fn parse_porcelain_v2_z(bytes: &[u8]) -> PorcelainStatus {
    let mut status = PorcelainStatus {
        headers: Vec::new(),
        entries: Vec::new(),
    };

    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        match record {
            [b'#', b' ', ..] => status.headers.push(record.to_vec()),
            [b'?', b' ', path @ ..] => status.entries.push(StatusEntry::Untracked(path.to_vec())),
            [b'!', b' ', path @ ..] => status.entries.push(StatusEntry::Ignored(path.to_vec())),
            _ => status.entries.push(StatusEntry::Tracked(record.to_vec())),
        }
    }
    status
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub oid: String,
    pub subject: Vec<u8>,
}

/// Parses NUL-delimited `OID`, `subject` pairs.
///
/// # Errors
///
/// Returns an error when a pair is incomplete or an OID is not UTF-8.
pub fn parse_commit_records(bytes: &[u8]) -> Result<Vec<CommitRecord>, &'static str> {
    let fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        return Err("commit output ended without a subject separator");
    }

    fields
        .chunks_exact(2)
        .map(|pair| {
            let oid = std::str::from_utf8(pair[0])
                .map_err(|_| "commit oid was not UTF-8")?
                .trim_start_matches('\n')
                .to_owned();
            Ok(CommitRecord {
                oid,
                subject: pair[1].to_vec(),
            })
        })
        .collect()
}

fn git_candidates() -> Vec<PathBuf> {
    let mut candidates = env::var_os("GITRONIMO_GIT")
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    candidates.push(PathBuf::from("git"));
    candidates.extend(MACOS_GIT_PATHS.map(PathBuf::from));
    candidates.push(PathBuf::from("/usr/bin/git"));
    candidates
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Read,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use super::{
        GitExecutable, StatusEntry, git_candidates, parse_commit_records, parse_porcelain_v2_z,
    };
    use app_core::{RepositoryDiscoverer, RepositoryOpenError, open_repository};
    use git_domain::RepositoryLocation;

    static NEXT_REPOSITORY: AtomicUsize = AtomicUsize::new(0);

    struct Repository {
        path: std::path::PathBuf,
        git: GitExecutable,
    }

    impl Repository {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "gitronimo-git-cli-{}-{}",
                std::process::id(),
                NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temporary repository directory should exist");
            let git =
                GitExecutable::discover().expect("Git should be installed for integration tests");
            let repository = Self { path, git };
            repository.success(["init", "--initial-branch=main"]);
            repository.success(["config", "user.email", "test@gitronimo.invalid"]);
            repository.success(["config", "user.name", "Gitronimo Test"]);
            repository
        }

        fn success<I, S>(&self, args: I)
        where
            I: IntoIterator<Item = S>,
            S: AsRef<std::ffi::OsStr>,
        {
            let output = self.git.run(&self.path, args).expect("Git should run");
            assert!(output.status.success(), "Git command failed: {output:?}");
        }

        fn commit(&self, subject: &str) {
            fs::write(self.path.join("fixture.txt"), subject).expect("fixture file should write");
            self.success(["add", "fixture.txt"]);
            self.success(["commit", "-m", subject]);
        }
    }

    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_git_and_runs_version() {
        let git =
            GitExecutable::discover().expect("Git should be discoverable outside a shell PATH");
        assert!(
            git.version()
                .expect("Git should report a version")
                .starts_with("git version ")
        );
        assert!(
            git_candidates()
                .iter()
                .any(|path| path == std::path::Path::new("/usr/bin/git"))
        );
    }

    #[test]
    fn parses_porcelain_v2_with_unusual_filenames() {
        let repository = Repository::new();
        let filename = "sp ace\tand\nunicode-é.txt";
        fs::write(repository.path.join(filename), "untracked")
            .expect("unusual filename should write");
        let output = repository
            .git
            .run(
                &repository.path,
                ["status", "--porcelain=v2", "--branch", "-z"],
            )
            .expect("status should run");
        let status = parse_porcelain_v2_z(&output.stdout);
        assert!(
            status
                .headers
                .iter()
                .any(|header| header.starts_with(b"# branch."))
        );
        assert!(
            status
                .entries
                .contains(&StatusEntry::Untracked(filename.as_bytes().to_vec()))
        );
    }

    #[test]
    fn loads_five_hundred_commit_records_with_explicit_separators() {
        let repository = Repository::new();
        for index in 0..500 {
            repository.commit(&format!("commit {index}"));
        }
        let output = repository
            .git
            .run(
                &repository.path,
                ["log", "-z", "--max-count=500", "--format=%H%x00%s"],
            )
            .expect("log should run");
        let commits = parse_commit_records(&output.stdout).expect("commit separators should parse");
        assert_eq!(commits.len(), 500);
        assert_eq!(commits[0].subject, b"commit 499");
    }

    #[test]
    fn discovers_nested_worktrees_and_rejects_bare_or_invalid_locations() {
        let repository = Repository::new();
        let nested = repository.path.join("nested");
        fs::create_dir_all(&nested).expect("nested directory should create");

        let location = repository
            .git
            .discover_repository(&nested)
            .expect("nested folder should resolve to its worktree");
        let RepositoryLocation::Worktree(worktree) = location else {
            panic!("fixture should be a working-tree repository");
        };
        assert_eq!(
            worktree.worktree_root,
            repository
                .path
                .canonicalize()
                .expect("fixture path should resolve")
        );
        assert!(worktree.git_dir.is_dir());
        assert_eq!(
            open_repository(&repository.git, &nested).expect("app core should open worktree"),
            worktree
        );

        let bare = repository.path.with_extension("bare.git");
        repository.success([
            "init",
            "--bare",
            bare.to_str().expect("temporary path is UTF-8"),
        ]);
        assert!(matches!(
            repository.git.discover_repository(&bare),
            Ok(RepositoryLocation::Bare { .. })
        ));
        assert!(matches!(
            open_repository(&repository.git, &bare),
            Err(RepositoryOpenError::BareRepository(_))
        ));
        assert!(matches!(
            repository
                .git
                .discover_repository(&nested.join("not-a-directory")),
            Err(RepositoryOpenError::NotDirectory(_))
        ));
        let outside = repository.path.with_extension("outside");
        fs::create_dir_all(&outside).expect("outside directory should create");
        assert!(matches!(
            RepositoryDiscoverer::discover_repository(&repository.git, &outside),
            Err(RepositoryOpenError::NotRepository(_))
        ));
        let _ = fs::remove_dir_all(bare);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn fetches_from_a_local_remote_and_cancels_a_running_git_process() {
        let source = Repository::new();
        source.commit("initial");
        let remote = source.path.with_extension("remote.git");
        source.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);

        let destination = Repository::new();
        destination.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let mut fetch = destination
            .git
            .start(&destination.path, ["fetch", "--progress", "origin"])
            .expect("fetch should start");
        let mut progress = String::new();
        fetch
            .take_stderr()
            .expect("fetch should expose stderr progress")
            .read_to_string(&mut progress)
            .expect("fetch progress should be readable");
        assert!(fetch.wait().expect("fetch should finish").success());
        assert!(!progress.is_empty(), "fetch should emit progress or status");

        let mut blocked = destination
            .git
            .start(&destination.path, ["hash-object", "--stdin"])
            .expect("long-running Git process should start");
        std::thread::sleep(Duration::from_millis(20));
        blocked
            .cancel()
            .expect("running Git process should be cancellable");
        assert!(
            !blocked
                .wait()
                .expect("cancelled process should exit")
                .success()
        );
        let _ = fs::remove_dir_all(remote);
    }
}
