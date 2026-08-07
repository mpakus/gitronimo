//! Safe adapter for the installed Git executable.

use std::{
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, ExitStatus, Output, Stdio},
};

use app_core::{RepositoryDiscoverer, RepositoryOpenError};
use git_domain::{
    BranchStatus, DiffFile, DiffHunk, DiffLine, DiffLineKind, FileStatus, GitPath, HeadStatus,
    RenameKind, RepositoryLocation, StatusEntry, SubmoduleState, UnifiedDiff, WorktreeRepository,
    WorktreeStatus,
};

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

    /// Reads the working-copy state using porcelain-v2 without decoding paths.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot run, rejects the request, or produces malformed porcelain.
    pub fn worktree_status(
        &self,
        repository: &WorktreeRepository,
        include_ignored: bool,
    ) -> Result<WorktreeStatus, GitStatusError> {
        let mut args = vec!["status", "--porcelain=v2", "--branch", "-z"];
        if include_ignored {
            args.push("--ignored=matching");
        }
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(GitStatusError::CommandFailed);
        }
        let mut status = parse_porcelain_v2_z(&output.stdout)?;

        let stashes = self.run(&repository.worktree_root, ["stash", "list", "-z"])?;
        if !stashes.status.success() {
            return Err(GitStatusError::CommandFailed);
        }
        status.stash_count = stashes
            .stdout
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .count()
            .try_into()
            .map_err(|_| GitStatusError::TooManyStashes)?;
        Ok(status)
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

#[derive(Debug)]
pub enum GitStatusError {
    Io(io::Error),
    CommandFailed,
    Parse(PorcelainParseError),
    TooManyStashes,
}

impl From<io::Error> for GitStatusError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<PorcelainParseError> for GitStatusError {
    fn from(error: PorcelainParseError) -> Self {
        Self::Parse(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PorcelainParseError {
    MalformedHeader,
    MalformedEntry,
    MissingRenameSource,
}

/// Parses NUL-delimited porcelain-v2 output without decoding repository paths.
///
/// # Errors
///
/// Returns an error when Git's stable porcelain record layout is malformed.
pub fn parse_porcelain_v2_z(bytes: &[u8]) -> Result<WorktreeStatus, PorcelainParseError> {
    let mut status = WorktreeStatus::default();
    let mut records = bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());

    while let Some(record) = records.next() {
        match record {
            [b'#', b' ', header @ ..] => parse_branch_header(header, &mut status.branch)?,
            [b'1', b' ', ..] => status.entries.push(parse_ordinary(record)?),
            [b'2', b' ', ..] => {
                let source_path = records
                    .next()
                    .ok_or(PorcelainParseError::MissingRenameSource)?;
                status.entries.push(parse_renamed(record, source_path)?);
            }
            [b'u', b' ', ..] => status.entries.push(parse_unmerged(record)?),
            [b'?', b' ', path @ ..] => status
                .entries
                .push(StatusEntry::Untracked(GitPath(path.to_vec()))),
            [b'!', b' ', path @ ..] => status
                .entries
                .push(StatusEntry::Ignored(GitPath(path.to_vec()))),
            _ => return Err(PorcelainParseError::MalformedEntry),
        }
    }
    Ok(status)
}

fn parse_branch_header(
    header: &[u8],
    branch: &mut BranchStatus,
) -> Result<(), PorcelainParseError> {
    let (key, value) = split_once(header, b' ').ok_or(PorcelainParseError::MalformedHeader)?;
    match key {
        b"branch.oid" => branch.oid = (value != b"(initial)").then(|| value.to_vec()),
        b"branch.head" => {
            branch.head = match value {
                b"(detached)" => HeadStatus::Detached,
                b"(initial)" => HeadStatus::Unborn,
                b"(unknown)" => HeadStatus::Unknown,
                _ => HeadStatus::Branch(GitPath(value.to_vec())),
            };
        }
        b"branch.upstream" => branch.upstream = Some(GitPath(value.to_vec())),
        b"branch.ab" => {
            let (ahead, behind) =
                split_once(value, b' ').ok_or(PorcelainParseError::MalformedHeader)?;
            branch.ahead = parse_divergence(ahead, b'+')?;
            branch.behind = parse_divergence(behind, b'-')?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_ordinary(record: &[u8]) -> Result<StatusEntry, PorcelainParseError> {
    let fields = fields(record, 9)?;
    Ok(StatusEntry::Ordinary {
        status: parse_status(fields[1])?,
        submodule: parse_submodule(fields[2]),
        path: GitPath(fields[8].to_vec()),
    })
}

fn parse_renamed(record: &[u8], source_path: &[u8]) -> Result<StatusEntry, PorcelainParseError> {
    let fields = fields(record, 10)?;
    let score = fields[8];
    let (&kind, score) = score
        .split_first()
        .ok_or(PorcelainParseError::MalformedEntry)?;
    Ok(StatusEntry::Renamed {
        status: parse_status(fields[1])?,
        submodule: parse_submodule(fields[2]),
        kind: match kind {
            b'R' => RenameKind::Rename,
            b'C' => RenameKind::Copy,
            _ => return Err(PorcelainParseError::MalformedEntry),
        },
        score: std::str::from_utf8(score)
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or(PorcelainParseError::MalformedEntry)?,
        path: GitPath(fields[9].to_vec()),
        source_path: GitPath(source_path.to_vec()),
    })
}

fn parse_unmerged(record: &[u8]) -> Result<StatusEntry, PorcelainParseError> {
    let fields = fields(record, 11)?;
    Ok(StatusEntry::Unmerged {
        status: parse_status(fields[1])?,
        submodule: parse_submodule(fields[2]),
        path: GitPath(fields[10].to_vec()),
    })
}

fn fields(record: &[u8], count: usize) -> Result<Vec<&[u8]>, PorcelainParseError> {
    let fields = record
        .splitn(count, |byte| *byte == b' ')
        .collect::<Vec<_>>();
    (fields.len() == count)
        .then_some(fields)
        .ok_or(PorcelainParseError::MalformedEntry)
}

fn parse_status(value: &[u8]) -> Result<FileStatus, PorcelainParseError> {
    let [index, worktree] = *value else {
        return Err(PorcelainParseError::MalformedEntry);
    };
    Ok(FileStatus([index, worktree]))
}

fn parse_submodule(value: &[u8]) -> SubmoduleState {
    match value {
        b"N..." => SubmoduleState::NotSubmodule,
        [b'S', commit, modified, untracked] => SubmoduleState::Changed {
            commit: *commit != b'.',
            modified: *modified != b'.',
            untracked: *untracked != b'.',
        },
        _ => SubmoduleState::Unknown(value.to_vec()),
    }
}

fn split_once(bytes: &[u8], delimiter: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == delimiter)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn parse_divergence(value: &[u8], sign: u8) -> Result<u32, PorcelainParseError> {
    if value.first() != Some(&sign) {
        return Err(PorcelainParseError::MalformedHeader);
    }
    std::str::from_utf8(&value[1..])
        .ok()
        .and_then(|number| number.parse().ok())
        .ok_or(PorcelainParseError::MalformedHeader)
}

/// Parses Git's unified diff text without decoding file paths or line content.
#[must_use]
pub fn parse_unified_diff(bytes: &[u8]) -> UnifiedDiff {
    let mut diff = UnifiedDiff::default();
    let mut current: Option<DiffFile> = None;

    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        if let Some(paths) = line.strip_prefix(b"diff --git ") {
            if let Some(file) = current.take() {
                diff.files.push(file);
            }
            let paths = split_once(paths, b' ');
            current = Some(DiffFile {
                old_path: paths
                    .and_then(|(old_path, _)| strip_diff_prefix(old_path))
                    .map(|path| GitPath(path.to_vec())),
                new_path: paths
                    .and_then(|(_, new_path)| strip_diff_prefix(new_path))
                    .map(|path| GitPath(path.to_vec())),
                ..Default::default()
            });
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if let Some(path) = line.strip_prefix(b"--- ") {
            file.old_path = strip_diff_prefix(path).map(|path| GitPath(path.to_vec()));
        } else if let Some(path) = line.strip_prefix(b"+++ ") {
            file.new_path = strip_diff_prefix(path).map(|path| GitPath(path.to_vec()));
        } else if let Some(path) = line.strip_prefix(b"rename from ") {
            file.old_path = Some(GitPath(path.to_vec()));
        } else if let Some(path) = line.strip_prefix(b"rename to ") {
            file.new_path = Some(GitPath(path.to_vec()));
        } else if line.starts_with(b"Binary files ") || line == b"GIT binary patch" {
            file.binary = true;
        } else if line.starts_with(b"@@ ") {
            file.hunks.push(DiffHunk {
                header: line.to_vec(),
                lines: Vec::new(),
            });
        } else if line == b"\\ No newline at end of file" {
            if let Some(hunk) = file.hunks.last_mut() {
                if let Some(last_line) = hunk.lines.last_mut() {
                    last_line.missing_final_newline = true;
                }
            }
        } else if let Some(hunk) = file.hunks.last_mut() {
            let (kind, content) = match line {
                [b' ', content @ ..] => (DiffLineKind::Context, content),
                [b'+', content @ ..] => (DiffLineKind::Addition, content),
                [b'-', content @ ..] => (DiffLineKind::Removal, content),
                _ => continue,
            };
            hunk.lines.push(DiffLine {
                kind,
                content: content.to_vec(),
                missing_final_newline: false,
            });
        }
    }
    if let Some(file) = current {
        diff.files.push(file);
    }
    diff
}

fn strip_diff_prefix(path: &[u8]) -> Option<&[u8]> {
    match path {
        b"/dev/null" => None,
        [b'a', b'/', path @ ..] | [b'b', b'/', path @ ..] => Some(path),
        path => Some(path),
    }
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
        GitExecutable, RenameKind, git_candidates, parse_commit_records, parse_porcelain_v2_z,
        parse_unified_diff,
    };
    use app_core::{RepositoryDiscoverer, RepositoryOpenError, open_repository};
    use git_domain::{
        DiffLineKind, GitPath, HeadStatus, RepositoryLocation, StatusEntry, SubmoduleState,
    };

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
    fn reads_status_with_unusual_filenames() {
        let repository = Repository::new();
        let filename = "sp ace\tand\nunicode-é.txt";
        fs::write(repository.path.join(filename), "untracked")
            .expect("unusual filename should write");
        let worktree = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree");
        let RepositoryLocation::Worktree(worktree) = worktree else {
            panic!("fixture should be a working tree");
        };
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should parse");
        assert!(matches!(status.branch.head, HeadStatus::Branch(_)));
        assert!(status.entries.contains(&StatusEntry::Untracked(GitPath(
            filename.as_bytes().to_vec()
        ))));
    }

    #[test]
    fn parses_every_porcelain_v2_status_record() {
        let output = b"# branch.oid abcdef\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +3 -2\0\
1 M. N... 100644 100644 100644 abc def ordinary.txt\0\
2 R. SCMU 100644 100644 100644 abc def R087 renamed.txt\0original.txt\0\
2 C. N... 100644 100644 100644 abc def C100 copied.txt\0source.txt\0\
u UU N... 100644 100644 100644 100644 abc def 123 conflict.txt\0\
? untracked\xff.txt\0! ignored.txt\0";
        let status = parse_porcelain_v2_z(output).expect("fixture should parse");

        assert_eq!(status.branch.oid, Some(b"abcdef".to_vec()));
        assert_eq!(
            status.branch.head,
            HeadStatus::Branch(GitPath(b"main".to_vec()))
        );
        assert_eq!(
            status.branch.upstream,
            Some(GitPath(b"origin/main".to_vec()))
        );
        assert_eq!((status.branch.ahead, status.branch.behind), (3, 2));
        assert_eq!(status.entries.len(), 6);
        assert!(matches!(
            &status.entries[0],
            StatusEntry::Ordinary { path, .. } if path == &GitPath(b"ordinary.txt".to_vec())
        ));
        assert!(matches!(
            &status.entries[1],
            StatusEntry::Renamed {
                kind: RenameKind::Rename,
                score: 87,
                submodule: SubmoduleState::Changed { commit: true, modified: true, untracked: true },
                path,
                source_path,
                ..
            } if path == &GitPath(b"renamed.txt".to_vec()) && source_path == &GitPath(b"original.txt".to_vec())
        ));
        assert!(matches!(
            &status.entries[2],
            StatusEntry::Renamed {
                kind: RenameKind::Copy,
                score: 100,
                ..
            }
        ));
        assert!(matches!(&status.entries[3], StatusEntry::Unmerged { .. }));
        assert!(status.entries.contains(&StatusEntry::Untracked(GitPath(
            b"untracked\xff.txt".to_vec()
        ))));
        assert!(
            status
                .entries
                .contains(&StatusEntry::Ignored(GitPath(b"ignored.txt".to_vec())))
        );
    }

    #[test]
    fn parses_unified_text_rename_binary_and_missing_newline() {
        let diff = parse_unified_diff(
            b"diff --git a/old.txt b/new.txt\n\
similarity index 100%\n\
rename from old.txt\n\
rename to new.txt\n\
@@ -1 +1 @@\n\
-old\n\
\\ No newline at end of file\n\
+new\n\
\\ No newline at end of file\n\
diff --git a/image.png b/image.png\n\
Binary files a/image.png and b/image.png differ\n",
        );
        assert_eq!(diff.files.len(), 2);
        assert_eq!(diff.files[0].old_path, Some(GitPath(b"old.txt".to_vec())));
        assert_eq!(diff.files[0].new_path, Some(GitPath(b"new.txt".to_vec())));
        assert_eq!(diff.files[0].hunks[0].lines[0].kind, DiffLineKind::Removal);
        assert!(diff.files[0].hunks[0].lines[0].missing_final_newline);
        assert_eq!(diff.files[0].hunks[0].lines[1].kind, DiffLineKind::Addition);
        assert!(diff.files[0].hunks[0].lines[1].missing_final_newline);
        assert!(diff.files[1].binary);
    }

    #[test]
    fn parses_detached_head_and_counts_stashes() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "stash me").expect("fixture should write");
        repository.success(["stash", "push", "-m", "fixture"]);
        repository.success(["checkout", "--detach"]);
        fs::write(repository.path.join(".gitignore"), "ignored.txt\n")
            .expect("ignore file should write");
        fs::write(repository.path.join("ignored.txt"), "ignored")
            .expect("ignored file should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let without_ignored = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should parse");
        assert_eq!(without_ignored.stash_count, 1);
        assert_eq!(without_ignored.branch.head, HeadStatus::Detached);
        assert!(
            !without_ignored
                .entries
                .iter()
                .any(|entry| matches!(entry, StatusEntry::Ignored(_)))
        );

        let with_ignored = repository
            .git
            .worktree_status(&worktree, true)
            .expect("ignored status should parse");
        assert!(
            with_ignored
                .entries
                .contains(&StatusEntry::Ignored(GitPath(b"ignored.txt".to_vec())))
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
