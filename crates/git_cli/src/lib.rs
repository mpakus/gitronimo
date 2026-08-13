//! Safe adapter for the installed Git executable.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

use app_core::{RepositoryDiscoverer, RepositoryOpenError};
use git_domain::{
    BlameLine, BranchStatus, CommitIdentity, CommitSignature, CommitSignatureStatus, ConflictSide,
    DiffFile, DiffHunk, DiffLine, DiffLineKind, FileHistoryRequest, FileStatus, GitPath,
    HeadStatus, HistoryCommit, HistoryPage, HistoryReference, HistoryRequest, InProgressOperation,
    LfsEntry, NamedRef, RebaseAction, RebaseTodoItem, RecoveredBranchTip, RecoveryRecord,
    RefDecoration, RefSnapshot, ReflogEntry, ReflogRequest, Remote, RenameKind, RepositoryLocation,
    StashEntry, StatusEntry, SubmoduleEntry, SubmoduleState, TreeEntry, TreeEntryKind, UnifiedDiff,
    WorktreeEntry, WorktreeRepository, WorktreeStatus, parse_hunk_header, selected_lines_patch,
};

const MACOS_GIT_PATHS: [&str; 2] = ["/opt/homebrew/bin/git", "/usr/local/bin/git"];
pub const MAX_DISPLAY_DIFF_BYTES: usize = 1_000_000;
pub const MAX_PROCESS_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
static MESSAGE_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRequest {
    pub subject: String,
    pub body: String,
    pub amend: bool,
    pub sign_off: bool,
}

/// How far `git reset` moves the index and worktree relative to `HEAD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub(crate) const fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorIdentity {
    pub name: String,
    pub email: String,
}

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
        let output = self.version_with_timeout(VERSION_PROBE_TIMEOUT)?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(io::Error::other("Git did not accept --version"))
        }
    }

    fn version_with_timeout(&self, timeout: Duration) -> io::Result<Output> {
        let mut child = self
            .command(Path::new("."), ["--version"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Git executable did not respond to --version in time",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stderr"))?;
        Ok(Output {
            status,
            stdout: read_limited(stdout)?,
            stderr: read_limited(stderr)?,
        })
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
        self.run_env(directory, std::iter::empty(), args)
    }

    /// Initializes an empty worktree repository in `directory`.
    ///
    /// # Errors
    /// Returns Git's refusal when the directory is not writable or already has
    /// an incompatible repository layout.
    pub fn init_repository(&self, directory: &Path) -> Result<(), GitStatusError> {
        let output = self.run(directory, ["init", "--initial-branch=main", "."])?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(&output))
        }
    }

    /// Clones `source` into a typed destination path.
    ///
    /// # Errors
    /// Returns Git's refusal when the source cannot be read or the destination
    /// cannot be created.
    pub fn clone_repository(&self, source: &str, destination: &Path) -> Result<(), GitStatusError> {
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        let output = self.run(
            parent,
            [
                OsString::from("clone"),
                OsString::from(source),
                destination.as_os_str().to_os_string(),
            ],
        )?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(&output))
        }
    }

    fn run_env<I, E, S>(&self, directory: &Path, envs: E, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        E: IntoIterator<Item = (OsString, OsString)>,
    {
        let mut command = self.command(directory, args);
        for (key, value) in envs {
            command.env(key, value);
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stderr"))?;
        let stdout_reader = thread::spawn(move || read_limited(stdout));
        let stderr = match read_limited(stderr) {
            Ok(stderr) => stderr,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                return Err(error);
            }
        };
        let status = child.wait()?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| io::Error::other("Git stdout reader stopped unexpectedly"))??;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }

    fn run_with_stdin<I, S>(&self, directory: &Path, args: I, input: &[u8]) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut child = self
            .command(directory, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stdin"))?;
        stdin.write_all(input)?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Git did not expose stderr"))?;
        let stdout_reader = thread::spawn(move || read_limited(stdout));
        let stderr = read_limited(stderr)?;
        let status = child.wait()?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| io::Error::other("Git stdout reader stopped unexpectedly"))??;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
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
            return Err(command_error(&output));
        }
        let mut status = parse_porcelain_v2_z(&output.stdout)?;

        let stashes = self.run(&repository.worktree_root, ["stash", "list", "-z"])?;
        if !stashes.status.success() {
            return Err(command_error(&stashes));
        }
        status.stash_count = stashes
            .stdout
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .count()
            .try_into()
            .map_err(|_| GitStatusError::TooManyStashes)?;
        status.operation = self.in_progress_operation(repository);
        Ok(status)
    }

    /// Returns per-path addition/deletion counts from staged and unstaged diffs.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot run or rejects the request.
    pub fn diff_numstat(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<std::collections::HashMap<GitPath, (u64, u64)>, GitStatusError> {
        use std::collections::HashMap;

        let mut stats = HashMap::new();
        for args in [
            vec!["diff", "--numstat"],
            vec!["diff", "--cached", "--numstat"],
        ] {
            let output = self.run(&repository.worktree_root, args)?;
            if !output.status.success() {
                return Err(command_error(&output));
            }
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                if let Some((path, added, deleted)) = parse_numstat_line(line) {
                    let entry = stats.entry(path).or_insert((0_u64, 0_u64));
                    entry.0 = entry.0.saturating_add(added);
                    entry.1 = entry.1.saturating_add(deleted);
                }
            }
        }
        Ok(stats)
    }

    /// Detects a history-changing operation paused in the repository by looking for
    /// Git's per-worktree state files under the absolute Git directory. The target
    /// hex oid is read best-effort; a missing or unreadable marker reports `None`.
    #[must_use]
    pub fn in_progress_operation(&self, repository: &WorktreeRepository) -> InProgressOperation {
        let git_dir = &repository.git_dir;
        let merge_head = git_dir.join("MERGE_HEAD");
        if merge_head.exists() {
            return InProgressOperation::Merge {
                oid: read_state_oid(&merge_head),
            };
        }
        if git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir() {
            return InProgressOperation::Rebase;
        }
        let cherry_pick_head = git_dir.join("CHERRY_PICK_HEAD");
        if cherry_pick_head.exists() {
            return InProgressOperation::CherryPick {
                oid: read_state_oid(&cherry_pick_head),
            };
        }
        let revert_head = git_dir.join("REVERT_HEAD");
        if revert_head.exists() {
            return InProgressOperation::Revert {
                oid: read_state_oid(&revert_head),
            };
        }
        InProgressOperation::None
    }

    /// Captures the pre-operation refs a history-changing operation can move:
    /// HEAD's oid, HEAD's symbolic branch name, and every local branch tip.
    /// Callers record this before running a merge, cherry-pick, revert, rebase,
    /// or their abort/continue forms so the start state can be restored or
    /// described later.
    ///
    /// # Errors
    /// Returns an error when Git rejects the ref read.
    pub fn recovery_snapshot(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<RecoveryRecord, GitStatusError> {
        let head_output = self.run(&repository.worktree_root, ["rev-parse", "HEAD"])?;
        let old_head = head_output
            .status
            .success()
            .then(|| trim_oid(&head_output.stdout))
            .flatten();

        let name_output = self.run(
            &repository.worktree_root,
            ["symbolic-ref", "--quiet", "HEAD"],
        )?;
        let head_name = name_output
            .status
            .success()
            .then(|| trim_oid(&name_output.stdout))
            .flatten()
            .map(GitPath);

        let refs_output = self.run(
            &repository.worktree_root,
            [
                "for-each-ref",
                "refs/heads",
                "--format=%(refname) %(objectname)",
            ],
        )?;
        if !refs_output.status.success() {
            return Err(command_error(&refs_output));
        }
        let branch_tips = refs_output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|record| !record.is_empty())
            .filter_map(|record| {
                let separator = record.iter().position(|byte| *byte == b' ')?;
                Some(RecoveredBranchTip {
                    name: GitPath(record[..separator].to_vec()),
                    oid: record[separator + 1..].to_vec(),
                })
            })
            .collect();

        Ok(RecoveryRecord {
            old_head,
            head_name,
            branch_tips,
        })
    }

    /// Loads one staged or unstaged file diff for display.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot run or rejects the diff request.
    pub fn file_diff(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        staged: bool,
    ) -> Result<LoadedDiff, GitStatusError> {
        self.file_diff_with_limit(repository, path, staged, MAX_DISPLAY_DIFF_BYTES)
    }

    /// Loads a bounded history page without scanning the full repository history.
    ///
    /// # Errors
    /// Returns an error when Git rejects the requested revision or output is malformed.
    pub fn history_page(
        &self,
        repository: &WorktreeRepository,
        request: &HistoryRequest,
    ) -> Result<HistoryPage, GitStatusError> {
        let limit = request.limit.clamp(1, 500);
        let all_refs = matches!(request.reference, HistoryReference::All);
        let all_refs_skip = request
            .before
            .as_deref()
            .and_then(|cursor| cursor.strip_prefix("all:"))
            .and_then(|skip| skip.parse::<usize>().ok())
            .unwrap_or(0);
        let mut args = vec![
            OsString::from("log"),
            OsString::from("--no-decorate"),
            OsString::from(format!(
                "--max-count={}",
                limit + 1 + usize::from(!all_refs && request.before.is_some())
            )),
            OsString::from(
                "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%s%x00%b%x1e",
            ),
        ];
        if all_refs {
            args.push(OsString::from("--all"));
            if all_refs_skip > 0 {
                args.push(OsString::from(format!("--skip={all_refs_skip}")));
            }
        }
        let reference = request.before.as_deref().map_or_else(
            || match &request.reference {
                HistoryReference::Current => "HEAD".to_owned(),
                HistoryReference::All => "--all".to_owned(),
                HistoryReference::Named(name) => name.clone(),
            },
            ToOwned::to_owned,
        );
        if !all_refs {
            args.push(OsString::from(reference));
        }
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let mut commits = parse_history_records(&output.stdout)?;
        if !all_refs && request.before.is_some() && !commits.is_empty() {
            commits.remove(0);
        }
        let next_before = (commits.len() > limit).then(|| {
            if all_refs {
                format!("all:{}", all_refs_skip + limit)
            } else {
                commits[limit - 1].oid.clone()
            }
        });
        commits.truncate(limit);
        Ok(HistoryPage {
            commits,
            next_before,
        })
    }

    /// Reads the commit history of a single tracked path, newest first,
    /// following renames with `--follow`.
    ///
    /// # Errors
    /// Returns Git's refusal when the path is untracked or does not exist.
    pub fn file_history(
        &self,
        repository: &WorktreeRepository,
        request: &FileHistoryRequest,
    ) -> Result<Vec<HistoryCommit>, GitStatusError> {
        let limit = request.limit.clamp(1, 500);
        let mut args = vec![
            OsString::from("log"),
            OsString::from("--no-decorate"),
            OsString::from("--follow"),
            OsString::from(format!("--max-count={limit}")),
            OsString::from(
                "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%s%x00%b%x1e",
            ),
        ];
        args.push(OsString::from("--"));
        args.push(OsString::from_vec(request.path.0.clone()));
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_history_records(&output.stdout)
    }

    /// Attributes each line of a tracked path to the commit that introduced it,
    /// parsed from `git blame --line-porcelain`.
    ///
    /// # Errors
    /// Returns Git's refusal for an uncommitted, untracked, or absent path.
    pub fn blame(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
    ) -> Result<Vec<BlameLine>, GitStatusError> {
        let mut args = vec![OsString::from("blame"), OsString::from("--line-porcelain")];
        args.push(OsString::from("--"));
        args.push(OsString::from_vec(path.0.clone()));
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_blame(&output.stdout)
    }

    /// Loads a bounded unified diff between two refs (commits, branches, or
    /// tags). The two-dot semantics match `git diff A B`.
    ///
    /// # Errors
    /// Returns Git's refusal when either ref cannot be resolved.
    pub fn diff_refs(
        &self,
        repository: &WorktreeRepository,
        left: &str,
        right: &str,
    ) -> Result<LoadedDiff, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--binary",
                left,
                right,
            ],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let truncated = output.stdout.len() > MAX_DISPLAY_DIFF_BYTES;
        let bytes = &output.stdout[..output.stdout.len().min(MAX_DISPLAY_DIFF_BYTES)];
        Ok(LoadedDiff {
            diff: parse_unified_diff(bytes),
            truncated,
        })
    }

    /// Lists the entries of one tree level inside a commit. An empty `path`
    /// lists the root tree; a non-empty `path` lists that subdirectory.
    ///
    /// # Errors
    /// Returns Git's refusal when the oid is not a commit or tree.
    pub fn tree_entries(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<TreeEntry>, GitStatusError> {
        let mut treeish = oid.to_owned();
        if !path.0.is_empty() {
            treeish.push(':');
            treeish.push_str(&String::from_utf8_lossy(&path.0));
        }
        let output = self.run(&repository.worktree_root, ["ls-tree", "-z", &treeish])?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_ls_tree(&output.stdout)
    }

    /// Reads the bytes of a file at a commit, for browsing or export.
    ///
    /// # Errors
    /// Returns Git's refusal when the path is not a blob in that tree.
    pub fn file_at_revision(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<u8>, GitStatusError> {
        let mut args = vec![OsString::from("show"), OsString::from("--format=")];
        let mut revision = oid.to_owned();
        revision.push(':');
        revision.push_str(&String::from_utf8_lossy(&path.0));
        args.push(OsString::from(revision));
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        Ok(output.stdout)
    }

    /// Lists every worktree managed by the repository, main first.
    ///
    /// # Errors
    /// Returns Git's refusal when the worktree query fails.
    pub fn worktree_list(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<WorktreeEntry>, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            ["worktree", "list", "--porcelain"],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let entries = parse_worktree_list(&output.stdout);
        let dirty = self
            .run(
                &repository.worktree_root,
                ["status", "--porcelain", "--ignore-submodules"],
            )
            .ok()
            .is_some_and(|output| {
                output.status.success() && !output.stdout.iter().all(u8::is_ascii_whitespace)
            });
        Ok(entries
            .into_iter()
            .map(|mut entry| {
                if entry.main {
                    entry.dirty = dirty;
                }
                entry
            })
            .collect())
    }

    /// Creates a linked worktree at `path` with a new branch `branch`.
    ///
    /// # Errors
    /// Returns Git's refusal for an existing path or branch name.
    pub fn add_worktree(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        branch: &str,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![OsString::from("worktree"), OsString::from("add")];
        args.push(OsString::from_vec(path.0.clone()));
        args.push(OsString::from(format!("-b{branch}")));
        self.mutate(repository, args)
    }

    /// Removes a linked worktree; callers explicitly opt in to force removal.
    ///
    /// # Errors
    /// Returns Git's refusal for a dirty or active worktree unless `force`.
    pub fn remove_worktree(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        force: bool,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![OsString::from("worktree"), OsString::from("remove")];
        if force {
            args.push(OsString::from("--force"));
        }
        args.push(OsString::from_vec(path.0.clone()));
        self.mutate(repository, args)
    }

    /// Lists every submodule registered in `.gitmodules`.
    ///
    /// # Errors
    /// Returns Git's refusal when the submodule query fails.
    pub fn submodule_list(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<SubmoduleEntry>, GitStatusError> {
        let output = self.run(&repository.worktree_root, ["submodule", "status"])?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_submodule_status(&output.stdout)
    }

    /// Initializes and updates a submodule from its configured remote.
    ///
    /// # Errors
    /// Returns Git's actionable failure for an invalid submodule path or
    /// network/authentication problems.
    pub fn submodule_update(
        &self,
        repository: &WorktreeRepository,
        path: Option<&GitPath>,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![
            OsString::from("submodule"),
            OsString::from("update"),
            OsString::from("--init"),
        ];
        if let Some(path) = path {
            args.push(OsString::from("--"));
            args.push(OsString::from_vec(path.0.clone()));
        }
        self.mutate(repository, args)
    }

    /// Starts an interactive rebase onto `base`.
    ///
    /// The sequence editor is a no-op so the generated plan is applied without
    /// spawning an editor; the plan editor view can then adjust the todo during
    /// any pause (for example a conflict).
    ///
    /// # Errors
    /// Returns Git's actionable failure when the worktree is dirty, `base` is
    /// invalid, or there is nothing to rebase.
    pub fn start_rebase(
        &self,
        repository: &WorktreeRepository,
        base: &str,
    ) -> Result<(), GitStatusError> {
        let output = self.run_env(
            &repository.worktree_root,
            [
                ("GIT_SEQUENCE_EDITOR".into(), ":".into()),
                ("GIT_EDITOR".into(), "true".into()),
            ],
            ["rebase", "--interactive", base],
        )?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(&output))
        }
    }

    /// Reads the todo of the rebase currently in progress.
    ///
    /// # Errors
    /// Returns `NoOperationInProgress` when no interactive rebase is paused.
    pub fn rebase_plan(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<RebaseTodoItem>, GitStatusError> {
        let todo_path = repository
            .git_dir
            .join("rebase-merge")
            .join("git-rebase-todo");
        if !todo_path.is_file() {
            return Err(GitStatusError::NoOperationInProgress);
        }
        let bytes = fs::read(&todo_path)?;
        Ok(parse_rebase_todo(&bytes))
    }

    /// Writes an edited todo back to the paused rebase.
    ///
    /// # Errors
    /// Returns `NoOperationInProgress` when no interactive rebase is paused.
    pub fn save_rebase_plan(
        &self,
        repository: &WorktreeRepository,
        items: &[RebaseTodoItem],
    ) -> Result<(), GitStatusError> {
        let todo_path = repository
            .git_dir
            .join("rebase-merge")
            .join("git-rebase-todo");
        if !todo_path.is_file() {
            return Err(GitStatusError::NoOperationInProgress);
        }
        let mut bytes = Vec::new();
        for item in items {
            bytes.extend_from_slice(item.action.verb().as_bytes());
            if !item.arguments.is_empty() {
                bytes.push(b' ');
                bytes.extend_from_slice(item.arguments.as_bytes());
            }
            bytes.push(b'\n');
        }
        fs::write(&todo_path, bytes)?;
        Ok(())
    }

    /// Aborts the rebase in progress and returns to the pre-rebase state.
    ///
    /// # Errors
    /// Returns Git's refusal when no rebase is in progress.
    pub fn rebase_abort(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["rebase", "--abort"])
    }

    /// Skips the current patch of a paused rebase.
    ///
    /// # Errors
    /// Returns Git's refusal when no rebase is in progress.
    pub fn rebase_skip(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["rebase", "--skip"])
    }

    /// Folds the staged changes into `target` as a squash (with `message`) or a
    /// fixup (without one).
    ///
    /// `target` is resolved to a full oid before the fold so the autosquash
    /// rebase can replay `target..HEAD`; the sequence editor is a no-op so the
    /// fold applies without spawning an editor.
    ///
    /// # Errors
    /// Returns Git's refusal when nothing is staged, `target` is invalid, or
    /// the autosquash rebase conflicts.
    pub fn autosquash(
        &self,
        repository: &WorktreeRepository,
        target: &str,
        message: Option<&str>,
    ) -> Result<(), GitStatusError> {
        let resolve = self.run(&repository.worktree_root, ["rev-parse", target])?;
        if !resolve.status.success() {
            return Err(command_error(&resolve));
        }
        let oid = trim_oid(&resolve.stdout).ok_or(GitStatusError::ParseReflog)?;
        let oid_text = String::from_utf8_lossy(&oid).into_owned();
        let mut commit_args = vec![OsString::from("commit")];
        if let Some(message) = message {
            commit_args.push(OsString::from("--squash"));
            commit_args.push(OsString::from_vec(oid.clone()));
            commit_args.push(OsString::from("-m"));
            commit_args.push(OsString::from(message.to_owned()));
        } else {
            commit_args.push(OsString::from("--fixup"));
            commit_args.push(OsString::from_vec(oid.clone()));
        }
        let commit_output = self.run_env(
            &repository.worktree_root,
            [("GIT_EDITOR".into(), "true".into())],
            commit_args,
        )?;
        if !commit_output.status.success() {
            return Err(command_error(&commit_output));
        }
        let base = format!("{oid_text}^");
        let rebase_output = self.run_env(
            &repository.worktree_root,
            [
                ("GIT_SEQUENCE_EDITOR".into(), ":".into()),
                ("GIT_EDITOR".into(), "true".into()),
            ],
            ["rebase", "--autosquash", "--interactive", base.as_str()],
        )?;
        if rebase_output.status.success() {
            Ok(())
        } else {
            Err(command_error(&rebase_output))
        }
    }

    /// Removes `target` from the current branch by rebasing everything after it
    /// onto its parent.
    ///
    /// # Errors
    /// Returns Git's refusal when the worktree is dirty, `target` is invalid,
    /// or there is nothing after `target` to rebase.
    pub fn drop_commit(
        &self,
        repository: &WorktreeRepository,
        target: &str,
    ) -> Result<(), GitStatusError> {
        let parent = format!("{target}^");
        let output = self.run(
            &repository.worktree_root,
            ["rebase", "--onto", parent.as_str(), target],
        )?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(&output))
        }
    }

    /// Resolves a conflicted file to one side and stages it as resolved.
    ///
    /// # Errors
    /// Returns Git's refusal when the path is not conflicted or cannot be
    /// checked out.
    pub fn resolve_conflict(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        side: ConflictSide,
    ) -> Result<(), GitStatusError> {
        let flag = match side {
            ConflictSide::Ours => "--ours",
            ConflictSide::Theirs => "--theirs",
        };
        let mut checkout = vec![
            OsString::from("checkout"),
            OsString::from(flag),
            OsString::from("--"),
        ];
        checkout.push(OsString::from_vec(path.0.clone()));
        self.mutate(repository, checkout)?;
        let mut add = vec![OsString::from("add"), OsString::from("--")];
        add.push(OsString::from_vec(path.0.clone()));
        self.mutate(repository, add)
    }

    /// Reads the working-tree copy of a file, marker lines and all.
    ///
    /// # Errors
    /// Returns an I/O error when the file is not present.
    pub fn read_working_file(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
    ) -> Result<Vec<u8>, GitStatusError> {
        let absolute = repository
            .worktree_root
            .join(PathBuf::from(String::from_utf8_lossy(&path.0).into_owned()));
        fs::read(&absolute).map_err(GitStatusError::Io)
    }

    /// Names the merge tool for `git mergetool` and disables the `.orig`
    /// backup files it leaves behind.
    ///
    /// # Errors
    /// Returns Git's refusal when the config cannot be written.
    pub fn set_merge_tool(
        &self,
        repository: &WorktreeRepository,
        tool: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["config", "merge.tool", tool])?;
        self.mutate(repository, ["config", "mergetool.keepBackup", "false"])
    }

    /// Launches the configured (or named) merge tool on every conflicted file,
    /// or on a single path.
    ///
    /// # Errors
    /// Returns Git's refusal when the tool is not configured or the invocation
    /// fails.
    pub fn run_merge_tool(
        &self,
        repository: &WorktreeRepository,
        tool: Option<&str>,
        path: Option<&GitPath>,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![OsString::from("mergetool"), OsString::from("--no-prompt")];
        if let Some(tool) = tool {
            args.push(OsString::from("-t"));
            args.push(OsString::from(tool));
        }
        if let Some(path) = path {
            args.push(OsString::from("--"));
            args.push(OsString::from_vec(path.0.clone()));
        }
        self.mutate(repository, args)
    }

    /// Reports the signature status and signer of a commit.
    ///
    /// # Errors
    /// Returns Git's refusal when `oid` does not resolve.
    pub fn commit_signature(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<CommitSignature, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            ["show", "--no-patch", "--format=%G?%x00%GS", oid],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        Ok(parse_signature(&output.stdout))
    }

    /// Resolves the abbreviated-to-full object id of `HEAD`.
    ///
    /// # Errors
    /// Returns an error when the repository has no `HEAD` commit.
    pub fn head_oid(&self, repository: &WorktreeRepository) -> Result<String, GitStatusError> {
        let output = self.run(&repository.worktree_root, ["rev-parse", "HEAD"])?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        trim_oid(&output.stdout)
            .map(|oid| String::from_utf8_lossy(&oid).into_owned())
            .ok_or(GitStatusError::ParseReflog)
    }

    /// Loads the short oid, subject, and body of `HEAD` for amend UI.
    ///
    /// # Errors
    /// Returns an error when the repository has no `HEAD` commit or output is malformed.
    pub fn head_commit_summary(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<HeadCommitSummary, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            ["log", "-1", "--format=%h%x00%s%x00%b"],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_head_commit_summary(&output.stdout).ok_or(GitStatusError::ParseReflog)
    }

    /// Lists every tracked path in the index, NUL-delimited.
    ///
    /// # Errors
    /// Returns an error when Git refuses to read the index.
    pub fn tracked_files(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<GitPath>, GitStatusError> {
        let output = self.run(&repository.worktree_root, ["ls-files", "-z"])?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        Ok(parse_nul_paths(&output.stdout))
    }

    /// Loads ref decorations independently from history records.
    ///
    /// # Errors
    /// Returns an error when Git rejects the ref query or output is malformed.
    pub fn ref_decorations(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<RefDecoration>, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            [
                "for-each-ref",
                "--format=%(refname:short)%00%(objectname)%00",
            ],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_ref_decorations(&output.stdout)
    }

    /// Loads branches, tags, and configured remotes without parsing presentation output.
    ///
    /// # Errors
    /// Returns an error when Git rejects a ref or configuration query.
    pub fn ref_snapshot(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<RefSnapshot, GitStatusError> {
        let local_heads = self.run(
            &repository.worktree_root,
            [
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(upstream:trackshort)%00",
                "refs/heads",
            ],
        )?;
        if !local_heads.status.success() {
            return Err(command_error(&local_heads));
        }
        let other_refs = self.run(
            &repository.worktree_root,
            [
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00",
                "refs/remotes",
                "refs/tags",
            ],
        )?;
        if !other_refs.status.success() {
            return Err(command_error(&other_refs));
        }
        let remotes = self.run(
            &repository.worktree_root,
            ["config", "--null", "--get-regexp", "^remote\\..*\\.url$"],
        )?;
        if !remotes.status.success() && remotes.status.code() != Some(1) {
            return Err(command_error(&remotes));
        }
        parse_ref_snapshot(&local_heads.stdout, &other_refs.stdout, &remotes.stdout)
    }

    /// Checks out an existing branch through Git's safe switch command.
    ///
    /// # Errors
    /// Returns Git's actionable failure, including dirty-worktree rejection.
    pub fn checkout_branch(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["switch", branch])
    }

    /// Checks out a commit in detached HEAD state (`git switch --detach`).
    ///
    /// # Errors
    /// Returns Git's actionable failure, including dirty-worktree rejection.
    pub fn checkout_detached(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["switch", "--detach", oid])
    }

    /// Moves `HEAD` (and optionally the index/worktree) to `oid`.
    ///
    /// # Errors
    /// Returns Git's refusal for an unknown commit or a blocked hard reset.
    pub fn reset_to(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        mode: ResetMode,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["reset", mode.flag(), oid])
    }

    /// Writes a single-commit patch for `oid` into `output_dir` via `format-patch`.
    ///
    /// # Errors
    /// Returns Git's refusal when the commit is unknown or the directory is unwritable.
    pub fn format_patch_to_dir(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        output_dir: &Path,
    ) -> Result<(), GitStatusError> {
        self.mutate(
            repository,
            [
                OsString::from("format-patch"),
                OsString::from("-1"),
                OsString::from(oid),
                OsString::from("-o"),
                output_dir.as_os_str().to_owned(),
            ],
        )
    }

    /// Creates a local branch that tracks `remote_branch` (e.g. `origin/feature`)
    /// and checks it out. Git derives the local name from the remote ref.
    ///
    /// # Errors
    /// Returns Git's refusal when the remote ref is unknown or a conflicting
    /// local branch already exists.
    pub fn checkout_tracking_branch(
        &self,
        repository: &WorktreeRepository,
        remote_branch: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["switch", "--track", remote_branch])
    }

    /// Creates and checks out a branch from HEAD or an explicit starting ref.
    ///
    /// # Errors
    /// Returns Git's actionable failure when the name or starting ref is invalid.
    pub fn create_branch(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
        start: Option<&str>,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![
            OsString::from("switch"),
            OsString::from("--create"),
            OsString::from(branch),
        ];
        if let Some(start) = start {
            args.push(OsString::from(start));
        }
        self.mutate(repository, args)
    }

    /// Renames a local branch without a shell command.
    ///
    /// # Errors
    /// Returns Git's actionable failure when the requested rename is invalid.
    pub fn rename_branch(
        &self,
        repository: &WorktreeRepository,
        old: &str,
        new: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["branch", "--move", old, new])
    }

    /// Deletes a local branch; callers must explicitly opt in to force deletion.
    ///
    /// # Errors
    /// Returns Git's refusal for an unmerged branch unless `force` is true.
    pub fn delete_branch(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
        force: bool,
    ) -> Result<(), GitStatusError> {
        self.mutate(
            repository,
            ["branch", if force { "-D" } else { "--delete" }, branch],
        )
    }

    /// Deletes a tag. `git branch --delete` cannot remove tags, so tag removal
    /// needs its own command.
    ///
    /// # Errors
    /// Returns Git's refusal when the tag does not exist.
    pub fn delete_tag(
        &self,
        repository: &WorktreeRepository,
        tag: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["tag", "--delete", tag])
    }

    /// Creates a lightweight tag at `start`.
    ///
    /// # Errors
    /// Returns Git's refusal when the name already exists or the ref is invalid.
    pub fn create_tag(
        &self,
        repository: &WorktreeRepository,
        tag: &str,
        start: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["tag", tag, start])
    }

    /// Points a local branch at a remote-tracking branch.
    ///
    /// # Errors
    /// Returns Git's refusal when either ref is unknown.
    pub fn set_branch_upstream(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
        upstream: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(
            repository,
            ["branch", &format!("--set-upstream-to={upstream}"), branch],
        )
    }

    /// Removes a local branch's upstream association.
    ///
    /// # Errors
    /// Returns Git's refusal when the branch has no upstream.
    pub fn unset_branch_upstream(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["branch", "--unset-upstream", branch])
    }

    /// Writes a zip archive of `reference` to `destination`.
    ///
    /// # Errors
    /// Returns Git's refusal when the ref is unknown or the path is unwritable.
    pub fn export_archive(
        &self,
        repository: &WorktreeRepository,
        reference: &str,
        destination: &Path,
    ) -> Result<(), GitStatusError> {
        self.mutate(
            repository,
            [
                OsString::from("archive"),
                OsString::from("--format=zip"),
                OsString::from("--output"),
                destination.as_os_str().to_owned(),
                OsString::from(reference),
            ],
        )
    }

    /// Reads a bounded reflog, newest entry first, from HEAD or the requested
    /// ref. The `old_oid` of each entry is derived from the following entry's
    /// `new_oid`, which git's reflog chain makes exact.
    ///
    /// # Errors
    /// Returns an error when Git rejects the reflog read.
    pub fn reflog(
        &self,
        repository: &WorktreeRepository,
        request: &ReflogRequest,
    ) -> Result<Vec<ReflogEntry>, GitStatusError> {
        let limit = request.limit.clamp(1, 500);
        let mut args = vec![
            OsString::from("reflog"),
            OsString::from(format!("--max-count={limit}")),
            OsString::from("--format=%H%x00%gD%x00%gs%x00%cn%x00%ce%x00%ct%x1e"),
        ];
        if let Some(reference) = &request.reference {
            args.push(OsString::from(reference));
        }
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let mut entries = parse_reflog_records(&output.stdout)?;
        for index in 0..entries.len() {
            if let Some(next) = entries.get(index + 1) {
                entries[index].old_oid = Some(next.new_oid.clone());
            }
        }
        Ok(entries)
    }

    /// Recreates a local branch pointing at the reflog oid selected by the
    /// user, restoring a branch whose commits are otherwise only reachable
    /// through the reflog. Git validates the branch name; an existing branch
    /// is rejected.
    ///
    /// # Errors
    /// Returns Git's refusal when the branch name is invalid or already exists.
    pub fn restore_branch_from_reflog(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
        oid: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["branch", branch, oid])
    }

    /// Fetches all configured refs from a selected remote.
    ///
    /// # Errors
    /// Returns Git's actionable authentication or transport failure.
    pub fn fetch_remote(
        &self,
        repository: &WorktreeRepository,
        remote: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["fetch", "--progress", remote])
    }

    /// Fetches a GitHub pull request head into a remote-tracking ref.
    ///
    /// # Errors
    /// Returns Git's actionable authentication or transport failure.
    pub fn fetch_pull_request(
        &self,
        repository: &WorktreeRepository,
        remote: &str,
        number: u64,
    ) -> Result<(), GitStatusError> {
        let refspec = format!("pull/{number}/head:refs/remotes/{remote}/pr/{number}");
        self.mutate(
            repository,
            [
                OsString::from("fetch"),
                OsString::from("--progress"),
                OsString::from(remote),
                OsString::from(refspec),
            ],
        )
    }

    /// Pulls the configured upstream for the current branch.
    ///
    /// # Errors
    /// Returns Git's actionable transport, authentication, or merge failure.
    pub fn pull_current(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["pull", "--progress"])
    }

    /// Pushes the current branch without force.
    ///
    /// # Errors
    /// Returns Git's actionable transport, authentication, or non-fast-forward failure.
    pub fn push_current(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["push", "--progress"])
    }

    /// Force-pushes the current branch only when the tracked remote ref has not changed.
    ///
    /// # Errors
    /// Returns Git's lease rejection or transport failure.
    pub fn push_current_with_lease(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["push", "--progress", "--force-with-lease"])
    }

    /// Publishes a branch and sets its upstream without force.
    ///
    /// # Errors
    /// Returns Git's actionable transport or authentication failure.
    pub fn publish_branch(
        &self,
        repository: &WorktreeRepository,
        remote: &str,
        branch: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(
            repository,
            ["push", "--progress", "--set-upstream", remote, branch],
        )
    }

    /// Lists the paths changed by one commit without parsing human-oriented output.
    ///
    /// # Errors
    /// Returns an error when Git rejects the commit object.
    pub fn commit_paths(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<Vec<GitPath>, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            ["show", "--format=", "--name-only", "-z", oid],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        Ok(output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| GitPath(path.to_vec()))
            .collect())
    }

    /// Loads a bounded unified diff for one commit.
    ///
    /// # Errors
    /// Returns an error when Git rejects the commit object.
    pub fn commit_diff(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<LoadedDiff, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            [
                "show",
                "--format=",
                "--no-ext-diff",
                "--no-textconv",
                "--binary",
                oid,
            ],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let truncated = output.stdout.len() > MAX_DISPLAY_DIFF_BYTES;
        let bytes = &output.stdout[..output.stdout.len().min(MAX_DISPLAY_DIFF_BYTES)];
        Ok(LoadedDiff {
            diff: parse_unified_diff(bytes),
            truncated,
        })
    }

    /// Loads one staged or unstaged file diff with a caller-selected display limit.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot run or rejects the diff request.
    pub fn file_diff_with_limit(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        staged: bool,
        limit: usize,
    ) -> Result<LoadedDiff, GitStatusError> {
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-textconv"),
            OsString::from("--binary"),
        ];
        if staged {
            args.push(OsString::from("--cached"));
        }
        args.push(OsString::from("--"));
        args.push(OsString::from_vec(path.0.clone()));
        let output = self.run(&repository.worktree_root, args)?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        let truncated = output.stdout.len() > limit;
        let bytes = &output.stdout[..output.stdout.len().min(limit)];
        Ok(LoadedDiff {
            diff: parse_unified_diff(bytes),
            truncated,
        })
    }

    /// Stages the supplied repository-relative paths.
    ///
    /// # Errors
    /// Returns an error when Git rejects the staging request.
    pub fn stage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitStatusError> {
        self.mutate_paths(repository, "add", paths)
    }

    /// Stages all working-copy changes, including deletions.
    ///
    /// # Errors
    /// Returns an error when Git rejects the staging request.
    pub fn stage_all(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["add", "-A"])
    }

    /// Stages one unstaged text hunk using Git's own patch validator.
    ///
    /// # Errors
    /// Returns an error when the path has no requested text hunk or Git rejects the patch.
    pub fn stage_hunk(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        hunk_index: usize,
    ) -> Result<(), GitStatusError> {
        let diff = self.run(
            &repository.worktree_root,
            [
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-textconv"),
                OsString::from("--binary"),
                OsString::from("--"),
                OsString::from_vec(path.0.clone()),
            ],
        )?;
        if !diff.status.success() {
            return Err(command_error(&diff));
        }
        let hunk_patch = single_hunk_patch(&diff.stdout, hunk_index)
            .ok_or(GitStatusError::PatchHunkUnavailable)?;
        let output = self.run_with_stdin(
            &repository.worktree_root,
            ["apply", "--cached", "--recount", "--whitespace=nowarn"],
            &hunk_patch,
        )?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    /// Unstages one staged text hunk without changing the working tree.
    ///
    /// # Errors
    /// Returns an error when the path has no requested text hunk or Git rejects the patch.
    pub fn unstage_hunk(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        hunk_index: usize,
    ) -> Result<(), GitStatusError> {
        let diff = self.run(
            &repository.worktree_root,
            [
                OsString::from("diff"),
                OsString::from("--cached"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-textconv"),
                OsString::from("--binary"),
                OsString::from("--"),
                OsString::from_vec(path.0.clone()),
            ],
        )?;
        if !diff.status.success() {
            return Err(command_error(&diff));
        }
        let hunk_patch = single_hunk_patch(&diff.stdout, hunk_index)
            .ok_or(GitStatusError::PatchHunkUnavailable)?;
        let output = self.run_with_stdin(
            &repository.worktree_root,
            [
                "apply",
                "--cached",
                "--reverse",
                "--recount",
                "--whitespace=nowarn",
            ],
            &hunk_patch,
        )?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    /// Stages only the selected change lines of an unstaged diff using Git's own
    /// patch validation. `selection` holds `(hunk_index, line_index)` pairs into
    /// the freshly requested diff for `path`.
    ///
    /// # Errors
    /// Returns an error when a selection is out of range or Git rejects the patch.
    pub fn stage_lines(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        selection: &[(usize, usize)],
    ) -> Result<(), GitStatusError> {
        let diff = self.unstaged_diff_for(path, repository)?;
        let parsed = parse_unified_diff(&diff.stdout);
        let lines_patch = patch_for_selected_lines(&diff.stdout, &parsed, selection)?;
        let output = self.run_with_stdin(
            &repository.worktree_root,
            ["apply", "--cached", "--recount", "--whitespace=nowarn"],
            &lines_patch,
        )?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    /// Discards only the selected change lines from the working tree, restoring the
    /// index content for those lines. `selection` holds `(hunk_index, line_index)`
    /// pairs into the freshly requested unstaged diff for `path`.
    ///
    /// # Errors
    /// Returns an error when a selection is out of range or Git rejects the patch.
    pub fn discard_lines(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        selection: &[(usize, usize)],
    ) -> Result<(), GitStatusError> {
        let diff = self.unstaged_diff_for(path, repository)?;
        let parsed = parse_unified_diff(&diff.stdout);
        let lines_patch = patch_for_selected_lines(&diff.stdout, &parsed, selection)?;
        let output = self.run_with_stdin(
            &repository.worktree_root,
            ["apply", "--reverse", "--recount", "--whitespace=nowarn"],
            &lines_patch,
        )?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    /// Discards one unstaged text hunk from the working tree, restoring the index
    /// content for that hunk's lines without touching any other hunk or the index.
    ///
    /// # Errors
    /// Returns an error when the path has no requested text hunk or Git rejects the patch.
    pub fn discard_hunk(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        hunk_index: usize,
    ) -> Result<(), GitStatusError> {
        let diff = self.unstaged_diff_for(path, repository)?;
        let hunk_patch = single_hunk_patch(&diff.stdout, hunk_index)
            .ok_or(GitStatusError::PatchHunkUnavailable)?;
        let output = self.run_with_stdin(
            &repository.worktree_root,
            ["apply", "--reverse", "--recount", "--whitespace=nowarn"],
            &hunk_patch,
        )?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    fn unstaged_diff_for(
        &self,
        path: &GitPath,
        repository: &WorktreeRepository,
    ) -> Result<Output, GitStatusError> {
        let diff = self.run(
            &repository.worktree_root,
            [
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-textconv"),
                OsString::from("--binary"),
                OsString::from("--"),
                OsString::from_vec(path.0.clone()),
            ],
        )?;
        if !diff.status.success() {
            return Err(command_error(&diff));
        }
        Ok(diff)
    }

    /// Stashes tracked changes, optionally including untracked files.
    ///
    /// # Errors
    /// Returns Git's refusal when there are no storable changes or the stash fails.
    pub fn create_stash(
        &self,
        repository: &WorktreeRepository,
        include_untracked: bool,
    ) -> Result<(), GitStatusError> {
        let mut args = vec![OsString::from("stash"), OsString::from("push")];
        if include_untracked {
            args.push(OsString::from("--include-untracked"));
        }
        self.mutate(repository, args)
    }

    /// Lists every stash newest-first, with its selector, oid, and subject.
    ///
    /// # Errors
    /// Returns Git's refusal when the stash query fails or the output cannot be parsed.
    pub fn stash_list(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<StashEntry>, GitStatusError> {
        let output = self.run(
            &repository.worktree_root,
            ["stash", "list", "--format=%gd%x00%H%x00%gs%x1e"],
        )?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_stash_records(&output.stdout)
    }

    /// Lists changed Git LFS paths using LFS's script-oriented porcelain format.
    ///
    /// # Errors
    /// Returns the LFS command failure or a malformed porcelain record.
    pub fn lfs_status(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Vec<LfsEntry>, GitStatusError> {
        let output = self.run(&repository.worktree_root, ["lfs", "status", "--porcelain"])?;
        if !output.status.success() {
            return Err(command_error(&output));
        }
        parse_lfs_status(&output.stdout)
    }

    /// Applies the named stash (e.g. `stash@{0}`) while retaining its recovery entry.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn apply_stash(
        &self,
        repository: &WorktreeRepository,
        reference: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "apply", reference])
    }

    /// Applies and removes the named stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn pop_stash(
        &self,
        repository: &WorktreeRepository,
        reference: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "pop", reference])
    }

    /// Removes the named stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's missing-stash failure.
    pub fn drop_stash(
        &self,
        repository: &WorktreeRepository,
        reference: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "drop", reference])
    }

    /// Applies the latest stash while retaining its recovery entry.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn apply_latest_stash(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<(), GitStatusError> {
        self.apply_stash(repository, "stash@{0}")
    }

    /// Applies and removes the latest stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn pop_latest_stash(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.pop_stash(repository, "stash@{0}")
    }

    /// Removes the latest stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's missing-stash failure.
    pub fn drop_latest_stash(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.drop_stash(repository, "stash@{0}")
    }

    /// Merges the named branch into the current branch. A conflicting merge leaves
    /// the repository paused with `MERGE_HEAD`; callers detect that with
    /// `in_progress_operation` and either abort or resolve, stage, and continue.
    ///
    /// # Errors
    /// Returns Git's conflict, refusal, or missing-branch failure.
    pub fn merge_branch(
        &self,
        repository: &WorktreeRepository,
        branch: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["merge", branch])
    }

    /// Aborts an in-progress merge, restoring the pre-merge working tree and index.
    ///
    /// # Errors
    /// Returns Git's refusal when no merge is in progress.
    pub fn abort_merge(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["merge", "--abort"])
    }

    /// Cherry-picks one commit onto the current branch. A conflict leaves the
    /// repository paused with `CHERRY_PICK_HEAD`.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-commit failure.
    pub fn cherry_pick(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["cherry-pick", oid])
    }

    /// Aborts an in-progress cherry-pick.
    ///
    /// # Errors
    /// Returns Git's refusal when no cherry-pick is in progress.
    pub fn abort_cherry_pick(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["cherry-pick", "--abort"])
    }

    /// Reverts one commit onto the current branch without opening an editor. A
    /// conflict leaves the repository paused with `REVERT_HEAD`.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-commit failure.
    pub fn revert_commit(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["revert", "--no-edit", oid])
    }

    /// Aborts an in-progress revert.
    ///
    /// # Errors
    /// Returns Git's refusal when no revert is in progress.
    pub fn abort_revert(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["revert", "--abort"])
    }

    /// Rebases the current branch onto the named base. A conflict pauses the
    /// rebase; `continue_operation` resumes it after the caller resolves and stages.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-base failure.
    pub fn rebase_branch(
        &self,
        repository: &WorktreeRepository,
        base: &str,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["rebase", base])
    }

    /// Aborts an in-progress rebase, returning to the original branch and commit.
    ///
    /// # Errors
    /// Returns Git's refusal when no rebase is in progress.
    pub fn abort_rebase(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["rebase", "--abort"])
    }

    /// Finishes a paused history operation after the caller resolved and staged
    /// every conflict. Merge continues with `git commit --no-edit` (reusing the
    /// recorded merge message); cherry-pick, revert, and rebase continue with their
    /// own `--continue` forms, and rebase never opens an editor.
    ///
    /// # Errors
    /// Returns Git's refusal when nothing is in progress or conflicts remain unstaged.
    pub fn continue_operation(
        &self,
        repository: &WorktreeRepository,
        operation: &InProgressOperation,
    ) -> Result<(), GitStatusError> {
        match operation {
            InProgressOperation::Merge { .. } => self.mutate(repository, ["commit", "--no-edit"]),
            InProgressOperation::CherryPick { .. } => {
                self.mutate(repository, ["cherry-pick", "--continue", "--no-edit"])
            }
            InProgressOperation::Revert { .. } => {
                self.mutate(repository, ["revert", "--continue", "--no-edit"])
            }
            InProgressOperation::Rebase => {
                let output = self.run_env(
                    &repository.worktree_root,
                    [("GIT_EDITOR".into(), "true".into())],
                    ["rebase", "--continue"],
                )?;
                output
                    .status
                    .success()
                    .then_some(())
                    .ok_or_else(|| command_error(&output))
            }
            InProgressOperation::None => Err(GitStatusError::NoOperationInProgress),
        }
    }

    /// Unstages the supplied paths, including in an unborn repository.
    ///
    /// # Errors
    /// Returns an error when Git rejects the unstaging request.
    pub fn unstage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitStatusError> {
        if self.has_head(repository)? {
            self.mutate_paths(repository, "reset", paths)
        } else {
            self.remove_from_index(repository, paths)
        }
    }

    /// Unstages every index entry, including in an unborn repository.
    ///
    /// # Errors
    /// Returns an error when Git rejects the unstaging request.
    pub fn unstage_all(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        if self.has_head(repository)? {
            self.mutate(repository, ["reset"])
        } else {
            self.mutate(repository, ["rm", "--cached", "-r", "."])
        }
    }

    /// Returns the identity Git will use for commits, if it is configured.
    ///
    /// # Errors
    /// Returns an error when Git cannot read repository configuration.
    pub fn author_identity(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<Option<AuthorIdentity>, GitStatusError> {
        let name = self.run(&repository.worktree_root, ["config", "--get", "user.name"])?;
        let email = self.run(&repository.worktree_root, ["config", "--get", "user.email"])?;
        if !name.status.success() || !email.status.success() {
            return Ok(None);
        }
        let name = String::from_utf8_lossy(&name.stdout).trim().to_owned();
        let email = String::from_utf8_lossy(&email.stdout).trim().to_owned();
        (!name.is_empty() && !email.is_empty())
            .then_some(AuthorIdentity { name, email })
            .ok_or(GitStatusError::MissingIdentity)
            .map(Some)
    }

    /// Commits the staged index using a temporary message file.
    ///
    /// # Errors
    /// Returns an error for an invalid message, missing identity, or rejected commit.
    pub fn commit(
        &self,
        repository: &WorktreeRepository,
        request: &CommitRequest,
    ) -> Result<(), GitStatusError> {
        if request.subject.trim().is_empty() {
            return Err(GitStatusError::InvalidCommitMessage);
        }
        if self.author_identity(repository)?.is_none() {
            return Err(GitStatusError::MissingIdentity);
        }
        let message_path = write_commit_message(&request.subject, &request.body)?;
        let mut args = vec![
            OsString::from("commit"),
            OsString::from("--cleanup=verbatim"),
            OsString::from("-F"),
            message_path.clone().into_os_string(),
        ];
        if request.amend {
            args.push(OsString::from("--amend"));
        }
        if request.sign_off {
            args.push(OsString::from("--signoff"));
        }
        let result = self.mutate(repository, args);
        let _ = fs::remove_file(message_path);
        result
    }

    /// Discards only tracked paths by restoring their HEAD version.
    ///
    /// # Errors
    /// Returns an error for unsafe, untracked, or unsupported paths.
    pub fn discard_tracked_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitStatusError> {
        if !self.has_head(repository)? {
            return Err(GitStatusError::UnsupportedDiscard);
        }
        for path in paths {
            if path.0.starts_with(b"/")
                || path.0.split(|byte| *byte == b'/').any(|part| part == b"..")
            {
                return Err(GitStatusError::UnsafePath);
            }
            let output = self.run(
                &repository.worktree_root,
                [
                    OsString::from("ls-files"),
                    OsString::from("--error-unmatch"),
                    OsString::from("--"),
                    OsString::from_vec(path.0.clone()),
                ],
            )?;
            if !output.status.success() {
                return Err(GitStatusError::UntrackedDeletionRefused);
            }
        }
        self.mutate_paths(repository, "restore", paths)
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

    /// Starts Git with piped standard input and error streams for progress and cancellation.
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
            .stdout(Stdio::null())
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

    fn mutate<I, S>(&self, repository: &WorktreeRepository, args: I) -> Result<(), GitStatusError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run(&repository.worktree_root, args)?;
        output
            .status
            .success()
            .then_some(())
            .ok_or_else(|| command_error(&output))
    }

    fn mutate_paths(
        &self,
        repository: &WorktreeRepository,
        command: &str,
        paths: &[GitPath],
    ) -> Result<(), GitStatusError> {
        let mut args = vec![OsString::from(command)];
        if command == "reset" {
            args.push(OsString::from("HEAD"));
        }
        if command == "restore" {
            args.extend([
                OsString::from("--worktree"),
                OsString::from("--source=HEAD"),
            ]);
        }
        args.push(OsString::from("--"));
        args.extend(paths.iter().map(|path| OsString::from_vec(path.0.clone())));
        self.mutate(repository, args)
    }

    fn remove_from_index(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitStatusError> {
        let mut args = vec![
            OsString::from("rm"),
            OsString::from("--cached"),
            OsString::from("--"),
        ];
        args.extend(paths.iter().map(|path| OsString::from_vec(path.0.clone())));
        self.mutate(repository, args)
    }

    fn has_head(&self, repository: &WorktreeRepository) -> Result<bool, GitStatusError> {
        Ok(self
            .run(&repository.worktree_root, ["rev-parse", "--verify", "HEAD"])?
            .status
            .success())
    }
}

fn write_commit_message(subject: &str, body: &str) -> Result<PathBuf, GitStatusError> {
    let path = std::env::temp_dir().join(format!(
        "gitronimo-commit-{}-{}.txt",
        std::process::id(),
        MESSAGE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    writeln!(file, "{subject}")?;
    if !body.is_empty() {
        writeln!(file, "\n{body}")?;
    }
    Ok(path)
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

/// Reads Git progress without allowing it to consume unbounded memory.
///
/// # Errors
///
/// Returns an error when the stream exceeds [`MAX_PROCESS_OUTPUT_BYTES`] or cannot be read.
pub fn read_stderr_limited(stderr: ChildStderr) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&read_limited(stderr)?).into_owned())
}

/// Parses a Git `--progress` stderr line for a percentage value in `[0.0, 1.0]`.
#[must_use]
pub fn parse_git_progress_line(line: &str) -> Option<f32> {
    for token in line.split_whitespace() {
        if let Some(number) = token.strip_suffix('%')
            && let Ok(value) = number.parse::<f32>()
        {
            return Some((value / 100.0).clamp(0.0, 1.0));
        }
    }
    None
}

/// Parses one `git diff --numstat` output line into path and line counts.
#[must_use]
pub fn parse_numstat_line(line: &str) -> Option<(GitPath, u64, u64)> {
    let mut fields = line.split('\t');
    let added = fields.next()?.parse().ok()?;
    let deleted = fields.next()?.parse().ok()?;
    let path = fields.next()?;
    Some((GitPath(path.as_bytes().to_vec()), added, deleted))
}

fn read_limited(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > MAX_PROCESS_OUTPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Git process output exceeded the safety limit",
            ));
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

#[derive(Debug)]
pub enum GitStatusError {
    Io(io::Error),
    CommandFailed(String),
    Parse(PorcelainParseError),
    TooManyStashes,
    MissingIdentity,
    InvalidCommitMessage,
    UntrackedDeletionRefused,
    UnsafePath,
    UnsupportedDiscard,
    PatchHunkUnavailable,
    PatchLinesUnavailable,
    NoOperationInProgress,
    ParseHistory,
    ParseHistoryFields,
    ParseHistoryTimestamp,
    ParseReflog,
    ParseStash,
    ParseLfs,
}

fn command_error(output: &Output) -> GitStatusError {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let message = if message.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    } else {
        message
    };
    let message = if message.is_empty() {
        "Git command failed without an error message.".into()
    } else {
        message
    };
    GitStatusError::CommandFailed(message)
}

fn read_state_oid(path: &Path) -> Option<Vec<u8>> {
    let oid = fs::read(path).ok()?;
    let oid = oid
        .into_iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    (!oid.is_empty()).then_some(oid)
}

fn trim_oid(bytes: &[u8]) -> Option<Vec<u8>> {
    let oid = bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    (!oid.is_empty()).then_some(oid)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedDiff {
    pub diff: UnifiedDiff,
    pub truncated: bool,
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
    let mut hunk_old_line = 1u64;
    let mut hunk_new_line = 1u64;

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
            (hunk_old_line, hunk_new_line) = parse_hunk_header(line).unwrap_or((1, 1));
            file.hunks.push(DiffHunk {
                header: line.to_vec(),
                lines: Vec::new(),
            });
        } else if line == b"\\ No newline at end of file" {
            if let Some(hunk) = file.hunks.last_mut()
                && let Some(last_line) = hunk.lines.last_mut()
            {
                last_line.missing_final_newline = true;
            }
        } else if let Some(hunk) = file.hunks.last_mut() {
            let (kind, content) = match line {
                [b' ', content @ ..] => (DiffLineKind::Context, content),
                [b'+', content @ ..] => (DiffLineKind::Addition, content),
                [b'-', content @ ..] => (DiffLineKind::Removal, content),
                _ => continue,
            };
            let mut old_line = None;
            let mut new_line = None;
            match kind {
                DiffLineKind::Context => {
                    old_line = Some(hunk_old_line);
                    new_line = Some(hunk_new_line);
                    hunk_old_line += 1;
                    hunk_new_line += 1;
                }
                DiffLineKind::Addition => {
                    new_line = Some(hunk_new_line);
                    hunk_new_line += 1;
                }
                DiffLineKind::Removal => {
                    old_line = Some(hunk_old_line);
                    hunk_old_line += 1;
                }
            }
            hunk.lines.push(DiffLine {
                kind,
                content: content.to_vec(),
                missing_final_newline: false,
                old_line,
                new_line,
            });
        }
    }
    if let Some(file) = current {
        diff.files.push(file);
    }
    diff
}

fn single_hunk_patch(diff: &[u8], wanted_index: usize) -> Option<Vec<u8>> {
    let mut header = Vec::new();
    let mut patch = Vec::new();
    let mut hunk_index = 0;
    let mut in_wanted_hunk = false;
    let mut saw_hunk = false;

    for line in diff.split_inclusive(|byte| *byte == b'\n') {
        if line.starts_with(b"@@ ") {
            if in_wanted_hunk {
                break;
            }
            saw_hunk = true;
            in_wanted_hunk = hunk_index == wanted_index;
            hunk_index += 1;
            if in_wanted_hunk {
                patch.extend_from_slice(&header);
            }
        }
        if !saw_hunk {
            header.extend_from_slice(line);
        } else if in_wanted_hunk {
            patch.extend_from_slice(line);
        }
    }
    in_wanted_hunk.then_some(patch)
}

/// Builds a single-file patch containing only the selected change lines of the
/// parsed diff, keeping the raw file header and recomputing each affected hunk.
fn patch_for_selected_lines(
    raw_diff: &[u8],
    parsed: &UnifiedDiff,
    selection: &[(usize, usize)],
) -> Result<Vec<u8>, GitStatusError> {
    let file = parsed
        .files
        .first()
        .ok_or(GitStatusError::PatchLinesUnavailable)?;
    let mut header = Vec::new();
    for line in raw_diff.split_inclusive(|byte| *byte == b'\n') {
        if line.starts_with(b"@@ ") {
            break;
        }
        header.extend_from_slice(line);
    }

    let mut selected_hunks = Vec::new();
    for &(hunk_index, line_index) in selection {
        let hunk = file
            .hunks
            .get(hunk_index)
            .ok_or(GitStatusError::PatchLinesUnavailable)?;
        let line = hunk
            .lines
            .get(line_index)
            .ok_or(GitStatusError::PatchLinesUnavailable)?;
        if line.kind == DiffLineKind::Context {
            return Err(GitStatusError::PatchLinesUnavailable);
        }
        selected_hunks.push(hunk_index);
    }
    selected_hunks.sort_unstable();
    selected_hunks.dedup();

    let mut patch = Vec::new();
    for hunk_index in selected_hunks {
        let hunk = &file.hunks[hunk_index];
        let mut selected = Vec::new();
        for &(selected_hunk, line_index) in selection {
            if selected_hunk == hunk_index {
                selected.push(line_index);
            }
        }
        patch.extend_from_slice(
            &selected_lines_patch(hunk, &selected).ok_or(GitStatusError::PatchLinesUnavailable)?,
        );
    }
    if patch.is_empty() {
        return Err(GitStatusError::PatchLinesUnavailable);
    }
    let mut full_patch = header;
    full_patch.extend_from_slice(&patch);
    Ok(full_patch)
}

fn strip_diff_prefix(path: &[u8]) -> Option<&[u8]> {
    (path != b"/dev/null").then(|| {
        path.strip_prefix(b"a/")
            .or_else(|| path.strip_prefix(b"b/"))
            .unwrap_or(path)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub oid: String,
    pub subject: Vec<u8>,
}

/// Subject/body of `HEAD` for the Working Copy amend composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadCommitSummary {
    pub short_oid: String,
    pub subject: String,
    pub body: String,
}

fn parse_head_commit_summary(bytes: &[u8]) -> Option<HeadCommitSummary> {
    let mut parts = bytes.splitn(3, |byte| *byte == 0);
    let short_oid = std::str::from_utf8(parts.next()?).ok()?.trim().to_owned();
    let subject = std::str::from_utf8(parts.next()?)
        .ok()?
        .trim_end_matches('\n')
        .to_owned();
    let body = parts
        .next()
        .map(|raw| {
            let text = String::from_utf8_lossy(raw);
            text.trim_end_matches('\n').to_owned()
        })
        .unwrap_or_default();
    if short_oid.is_empty() {
        return None;
    }
    Some(HeadCommitSummary {
        short_oid,
        subject,
        body,
    })
}

/// Parses NUL-delimited `OID`, `subject` pairs.
///
/// # Errors
///
/// Returns an error when a pair is incomplete or an OID is not UTF-8.
pub fn parse_commit_records(bytes: &[u8]) -> Result<Vec<CommitRecord>, &'static str> {
    let fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| field.iter().any(|byte| !byte.is_ascii_whitespace()))
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

fn parse_blame(bytes: &[u8]) -> Result<Vec<BlameLine>, GitStatusError> {
    let mut entries = Vec::new();
    let mut oid: Option<Vec<u8>> = None;
    let mut name = Vec::new();
    let mut email = Vec::new();
    let mut timestamp: i64 = 0;
    let mut header_started = false;
    for line in bytes.split(|byte| *byte == b'\n') {
        if let Some(content) = line.strip_prefix(b"\t") {
            if let Some(oid) = oid.take() {
                entries.push(BlameLine {
                    oid,
                    author: CommitIdentity {
                        name,
                        email,
                        timestamp,
                    },
                    content: content.to_vec(),
                });
            }
            name = Vec::new();
            email = Vec::new();
            header_started = false;
            continue;
        }
        if header_started {
            if let Some(rest) = line.strip_prefix(b"author ") {
                name = rest.to_vec();
            } else if let Some(rest) = line.strip_prefix(b"author-mail ") {
                email = rest
                    .strip_prefix(b"<")
                    .and_then(|rest| rest.strip_suffix(b">"))
                    .unwrap_or(rest)
                    .to_vec();
            } else if let Some(rest) = line.strip_prefix(b"author-time ") {
                timestamp = std::str::from_utf8(rest)
                    .map_err(|_| GitStatusError::ParseReflog)?
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| GitStatusError::ParseReflog)?;
            }
            continue;
        }
        let mut fields = line.split(u8::is_ascii_whitespace);
        if let Some(hex) = fields.next()
            && hex.len() == 40
            && fields.next().is_some()
        {
            oid = Some(hex.to_vec());
            header_started = true;
        }
    }
    Ok(entries)
}

fn parse_rebase_todo(bytes: &[u8]) -> Vec<RebaseTodoItem> {
    let mut items = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.trim_ascii();
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let verb_end = line
            .iter()
            .position(u8::is_ascii_whitespace)
            .unwrap_or(line.len());
        let action = RebaseAction::from_verb(&line[..verb_end]);
        let arguments = String::from_utf8_lossy(line[verb_end..].trim_ascii()).into_owned();
        items.push(RebaseTodoItem { action, arguments });
    }
    items
}

fn parse_nul_paths(bytes: &[u8]) -> Vec<GitPath> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| GitPath(path.to_vec()))
        .collect()
}

fn parse_signature(bytes: &[u8]) -> CommitSignature {
    let record = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let mut fields = record.split(|byte| *byte == 0);
    let status = match fields.next().unwrap_or_default() {
        b"G" => CommitSignatureStatus::Good,
        b"B" => CommitSignatureStatus::Bad,
        b"U" => CommitSignatureStatus::Unknown,
        b"N" => CommitSignatureStatus::None,
        b"X" => CommitSignatureStatus::Expired,
        b"Y" => CommitSignatureStatus::GoodExpired,
        b"R" => CommitSignatureStatus::Revoked,
        b"E" => CommitSignatureStatus::Error,
        other => CommitSignatureStatus::Other(String::from_utf8_lossy(other).into_owned()),
    };
    let signer = fields
        .next()
        .map(|signer| String::from_utf8_lossy(signer).into_owned())
        .unwrap_or_default();
    CommitSignature { status, signer }
}

fn parse_submodule_status(bytes: &[u8]) -> Result<Vec<SubmoduleEntry>, GitStatusError> {
    let mut entries = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let flag = line[0];
        let rest = line
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .map(|index| &line[index..])
            .unwrap_or_default();
        let oid_end = rest
            .iter()
            .position(u8::is_ascii_whitespace)
            .ok_or(GitStatusError::ParseReflog)?;
        let oid = rest[..oid_end].to_vec();
        let rest = rest[oid_end..].trim_ascii_start();
        let path_end = rest
            .iter()
            .position(u8::is_ascii_whitespace)
            .unwrap_or(rest.len());
        let path = GitPath(rest[..path_end].to_vec());
        let description = String::from_utf8_lossy(rest[path_end..].trim_ascii()).into_owned();
        entries.push(SubmoduleEntry {
            path,
            flag,
            oid,
            description,
        });
    }
    Ok(entries)
}

fn parse_worktree_list(bytes: &[u8]) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if let Some(path) = line.strip_prefix(b"worktree ") {
            entries.push(WorktreeEntry {
                path: GitPath(path.to_vec()),
                head: Vec::new(),
                branch: None,
                dirty: false,
                main: false,
            });
        } else if let Some(entry) = entries.last_mut() {
            if let Some(head) = line.strip_prefix(b"HEAD ") {
                entry.head = head.to_vec();
            } else if let Some(branch) = line.strip_prefix(b"branch refs/heads/") {
                entry.branch = Some(GitPath(branch.to_vec()));
            } else if line == b"detached" {
                entry.branch = None;
            }
        }
    }
    if let Some(main) = entries.first_mut() {
        main.main = true;
    }
    entries
}

fn parse_ls_tree(bytes: &[u8]) -> Result<Vec<TreeEntry>, GitStatusError> {
    let mut entries = Vec::new();
    for record in bytes.split(|byte| *byte == 0).filter(|r| !r.is_empty()) {
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(GitStatusError::ParseReflog)?;
        let meta = &record[..separator];
        let name = &record[separator + 1..];
        let mut fields = meta.split(|byte| *byte == b' ');
        let mode = String::from_utf8_lossy(fields.next().unwrap_or_default()).into_owned();
        let kind = match fields.next() {
            Some(b"blob") => TreeEntryKind::Blob,
            Some(b"tree") => TreeEntryKind::Tree,
            Some(b"commit") => TreeEntryKind::Commit,
            _ => return Err(GitStatusError::ParseReflog),
        };
        let oid = fields.next().unwrap_or_default().to_vec();
        entries.push(TreeEntry {
            name: GitPath(name.to_vec()),
            kind,
            oid,
            mode,
        });
    }
    Ok(entries)
}

fn parse_lfs_status(bytes: &[u8]) -> Result<Vec<LfsEntry>, GitStatusError> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
        .map(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.len() < 4 || line[2] != b' ' || line[3..].is_empty() {
                return Err(GitStatusError::ParseLfs);
            }
            Ok(LfsEntry {
                index_status: line[0],
                worktree_status: line[1],
                path: GitPath(line[3..].to_vec()),
            })
        })
        .collect()
}

fn parse_stash_records(bytes: &[u8]) -> Result<Vec<StashEntry>, GitStatusError> {
    bytes
        .split(|byte| *byte == 0x1e)
        .filter(|record| record.iter().any(|byte| !byte.is_ascii_whitespace()))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
            if fields.len() < 3 {
                return Err(GitStatusError::ParseStash);
            }
            let reference = std::str::from_utf8(fields[0])
                .map_err(|_| GitStatusError::ParseStash)?
                .trim()
                .to_owned();
            let oid = std::str::from_utf8(fields[1])
                .map_err(|_| GitStatusError::ParseStash)?
                .trim()
                .to_owned();
            Ok(StashEntry {
                reference,
                oid,
                subject: fields[2].to_vec(),
            })
        })
        .collect()
}

fn parse_reflog_records(bytes: &[u8]) -> Result<Vec<ReflogEntry>, GitStatusError> {
    bytes
        .split(|byte| *byte == 0x1e)
        .filter(|record| record.iter().any(|byte| !byte.is_ascii_whitespace()))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
            if fields.len() < 6 {
                return Err(GitStatusError::ParseReflog);
            }
            let new_oid = fields[0].to_vec();
            let selector = std::str::from_utf8(fields[1])
                .map_err(|_| GitStatusError::ParseReflog)?
                .trim()
                .to_owned();
            let subject = std::str::from_utf8(fields[2])
                .map_err(|_| GitStatusError::ParseReflog)?
                .trim()
                .to_owned();
            let name = fields[3].to_vec();
            let email = fields[4].to_vec();
            let timestamp = std::str::from_utf8(fields[5])
                .map_err(|_| GitStatusError::ParseReflog)?
                .trim()
                .parse::<i64>()
                .map_err(|_| GitStatusError::ParseReflog)?;
            Ok(ReflogEntry {
                old_oid: None,
                new_oid,
                selector,
                identity: CommitIdentity {
                    name,
                    email,
                    timestamp,
                },
                subject,
            })
        })
        .collect()
}

fn parse_history_records(bytes: &[u8]) -> Result<Vec<HistoryCommit>, GitStatusError> {
    bytes
        .split(|byte| *byte == 0x1e)
        .filter(|record| record.iter().any(|byte| !byte.is_ascii_whitespace()))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let record = record.strip_suffix(b"\n").unwrap_or(record);
            let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
            if fields.len() < 10 {
                return Err(GitStatusError::ParseHistoryFields);
            }
            let oid = std::str::from_utf8(fields[0])
                .map_err(|_| GitStatusError::ParseHistoryFields)?
                .to_owned();
            let parents = std::str::from_utf8(fields[1])
                .map_err(|_| GitStatusError::ParseHistoryFields)?
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect();
            let timestamp = |field: &[u8]| {
                std::str::from_utf8(field)
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .ok_or(GitStatusError::ParseHistoryTimestamp)
            };
            Ok(HistoryCommit {
                oid,
                parents,
                author: CommitIdentity {
                    name: fields[2].to_vec(),
                    email: fields[3].to_vec(),
                    timestamp: timestamp(fields[4])?,
                },
                committer: CommitIdentity {
                    name: fields[5].to_vec(),
                    email: fields[6].to_vec(),
                    timestamp: timestamp(fields[7])?,
                },
                subject: fields[8].to_vec(),
                body: fields[9..].join(&0),
            })
        })
        .collect()
}

fn parse_ref_decorations(bytes: &[u8]) -> Result<Vec<RefDecoration>, GitStatusError> {
    let fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| field.iter().any(|byte| !byte.is_ascii_whitespace()))
        .collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        return Err(GitStatusError::ParseHistory);
    }
    fields
        .chunks_exact(2)
        .map(|pair| {
            Ok(RefDecoration {
                name: pair[0].to_vec(),
                target: std::str::from_utf8(pair[1])
                    .map_err(|_| GitStatusError::ParseHistory)?
                    .to_owned(),
            })
        })
        .collect()
}

fn parse_ref_ahead_behind(raw: &[u8]) -> Result<(u32, u32), GitStatusError> {
    if raw.is_empty() || raw == b"=" {
        return Ok((0, 0));
    }
    let value = std::str::from_utf8(raw).map_err(|_| GitStatusError::ParseHistory)?;
    let mut ahead = 0;
    let mut behind = 0;
    for part in value.split_whitespace() {
        if let Some(count) = part.strip_prefix('+') {
            ahead = count.parse().map_err(|_| GitStatusError::ParseHistory)?;
        } else if let Some(count) = part.strip_prefix('-') {
            behind = count.parse().map_err(|_| GitStatusError::ParseHistory)?;
        }
    }
    Ok((ahead, behind))
}

fn parse_ref_snapshot(
    local_heads: &[u8],
    other_refs: &[u8],
    remotes: &[u8],
) -> Result<RefSnapshot, GitStatusError> {
    let mut snapshot = RefSnapshot::default();
    append_local_head_refs(local_heads, &mut snapshot)?;
    append_other_named_refs(other_refs, &mut snapshot)?;
    for entry in remotes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let (key, url) = entry.split_at(
            entry
                .iter()
                .position(|byte| *byte == b'\n')
                .ok_or(GitStatusError::ParseHistory)?,
        );
        let name = key
            .strip_prefix(b"remote.")
            .and_then(|key| key.strip_suffix(b".url"))
            .ok_or(GitStatusError::ParseHistory)?;
        snapshot.remotes.push(Remote {
            name: GitPath(name.to_vec()),
            fetch_url: url[1..].to_vec(),
        });
    }
    Ok(snapshot)
}

fn split_nul_fields(raw: &[u8]) -> Vec<Vec<u8>> {
    let without_newlines: Vec<u8> = raw.iter().copied().filter(|byte| *byte != b'\n').collect();
    let mut fields: Vec<Vec<u8>> = without_newlines
        .split(|byte| *byte == 0)
        .map(<[u8]>::to_vec)
        .collect();
    if fields.last().is_some_and(Vec::is_empty) {
        fields.pop();
    }
    fields
}

fn append_local_head_refs(
    local_heads: &[u8],
    snapshot: &mut RefSnapshot,
) -> Result<(), GitStatusError> {
    let fields = split_nul_fields(local_heads);
    if !fields.is_empty() && !fields.len().is_multiple_of(4) {
        return Err(GitStatusError::ParseHistory);
    }
    for chunk in fields.chunks_exact(4) {
        let name = chunk[0].trim_ascii_start();
        if !name.starts_with(b"refs/heads/") {
            return Err(GitStatusError::ParseHistory);
        }
        let target = std::str::from_utf8(&chunk[1])
            .map_err(|_| GitStatusError::ParseHistory)?
            .to_owned();
        let upstream_raw =
            std::str::from_utf8(&chunk[2]).map_err(|_| GitStatusError::ParseHistory)?;
        let upstream = (!upstream_raw.is_empty()).then(|| upstream_raw.to_owned());
        let (ahead, behind) = parse_ref_ahead_behind(&chunk[3])?;
        snapshot.local_branches.push(NamedRef {
            name: GitPath(name[b"refs/heads/".len()..].to_vec()),
            target,
            upstream,
            ahead,
            behind,
        });
    }
    Ok(())
}

fn append_other_named_refs(
    other_refs: &[u8],
    snapshot: &mut RefSnapshot,
) -> Result<(), GitStatusError> {
    let fields = split_nul_fields(other_refs);
    if !fields.is_empty() && !fields.len().is_multiple_of(2) {
        return Err(GitStatusError::ParseHistory);
    }
    for chunk in fields.chunks_exact(2) {
        let name = chunk[0].trim_ascii_start();
        let target = std::str::from_utf8(&chunk[1])
            .map_err(|_| GitStatusError::ParseHistory)?
            .to_owned();
        let named = |prefix: &[u8]| NamedRef {
            name: GitPath(name[prefix.len()..].to_vec()),
            target: target.clone(),
            upstream: None,
            ahead: 0,
            behind: 0,
        };
        if name.starts_with(b"refs/remotes/") {
            snapshot.remote_branches.push(named(b"refs/remotes/"));
        } else if name.starts_with(b"refs/tags/") {
            snapshot.tags.push(named(b"refs/tags/"));
        }
    }
    Ok(())
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
        io::{Cursor, Read},
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use super::{
        CommitRequest, GitExecutable, GitStatusError, MAX_PROCESS_OUTPUT_BYTES, RenameKind,
        ResetMode, git_candidates, parse_commit_records, parse_git_progress_line, parse_lfs_status,
        parse_nul_paths, parse_numstat_line, parse_porcelain_v2_z, parse_rebase_todo,
        parse_ref_ahead_behind, parse_ref_snapshot, parse_signature, parse_stash_records,
        parse_unified_diff, read_limited, trim_oid,
    };
    use app_core::{RepositoryDiscoverer, RepositoryOpenError, open_repository};
    use git_domain::{
        CommitSignature, CommitSignatureStatus, ConflictSide, DiffLineKind, FileHistoryRequest,
        GitPath, HeadStatus, HistoryReference, HistoryRequest, InProgressOperation, RebaseAction,
        ReflogRequest, RepositoryLocation, StatusEntry, SubmoduleState, TreeEntryKind,
    };

    static NEXT_REPOSITORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn process_output_reader_rejects_oversized_streams() {
        let output = vec![b'x'; MAX_PROCESS_OUTPUT_BYTES + 1];
        let error = read_limited(Cursor::new(output)).expect_err("output should be bounded");
        assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge);
    }

    #[test]
    fn parses_ref_ahead_behind_field() {
        assert_eq!(parse_ref_ahead_behind(b"").expect("empty"), (0, 0));
        assert_eq!(parse_ref_ahead_behind(b"+3 -2").expect("diverged"), (3, 2));
    }

    #[test]
    fn parses_ref_snapshot_with_upstream_tracking() {
        let local_heads = b"refs/heads/main\0abc123\0origin/main\0+1 -2\0";
        let other_refs = b"refs/remotes/origin/main\0def456\0refs/tags/v1\0tag123\0";
        let snapshot =
            parse_ref_snapshot(local_heads, other_refs, b"").expect("snapshot should parse");
        let main = snapshot
            .local_branches
            .iter()
            .find(|branch| branch.name.0 == b"main")
            .expect("main branch");
        assert_eq!(main.upstream.as_deref(), Some("origin/main"));
        assert_eq!((main.ahead, main.behind), (1, 2));
        assert_eq!(snapshot.remote_branches.len(), 1);
        assert_eq!(snapshot.tags.len(), 1);
    }

    #[test]
    fn parses_stash_list_records() {
        let records = parse_stash_records(
            b"stash@{0}\0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b\0WIP on main: 1a2b3c initial\0\x1e\
              stash@{1}\09f8e7d6c5b4a39281706050403020100ffeeddcc\0WIP on main: 1a2b3c first commit\0\x1e",
        )
        .expect("records should parse");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].reference, "stash@{0}");
        assert_eq!(records[0].oid, "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b");
        assert_eq!(records[0].subject, b"WIP on main: 1a2b3c initial".to_vec());
        assert_eq!(records[1].reference, "stash@{1}");
    }

    #[test]
    fn parses_stash_list_records_with_truncated_fields() {
        let error = parse_stash_records(b"stash@{0}\0only-two-fields").expect_err("should fail");
        assert!(matches!(error, GitStatusError::ParseStash));
    }

    #[test]
    fn parses_lfs_porcelain_status_and_raw_paths() {
        let entries = parse_lfs_status(b" M assets/large file.bin\nA  new.bin\r\n")
            .expect("LFS status should parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index_status, b' ');
        assert_eq!(entries[0].worktree_status, b'M');
        assert_eq!(entries[0].path, GitPath(b"assets/large file.bin".to_vec()));
        assert_eq!(entries[1].index_status, b'A');
        assert_eq!(entries[1].worktree_status, b' ');
    }

    #[test]
    fn rejects_malformed_lfs_porcelain_status() {
        let error = parse_lfs_status(b"M missing-status-column\n").expect_err("should fail");
        assert!(matches!(error, GitStatusError::ParseLfs));
    }

    #[test]
    fn parses_nul_delimited_tracked_paths() {
        assert_eq!(parse_nul_paths(b""), Vec::<GitPath>::new());
        assert_eq!(
            parse_nul_paths(b"a.txt\0dir/b.txt\0"),
            vec![GitPath(b"a.txt".to_vec()), GitPath(b"dir/b.txt".to_vec())]
        );
        assert_eq!(
            parse_nul_paths(b"only.txt\0"),
            vec![GitPath(b"only.txt".to_vec())]
        );
    }

    #[test]
    fn parses_commit_signature_records() {
        let parse = |payload: &[u8]| parse_signature(payload);
        let signature = |status, signer: &str| CommitSignature {
            status,
            signer: signer.to_owned(),
        };

        assert_eq!(
            parse(b"G\0Alice <alice@example.com>\n"),
            signature(CommitSignatureStatus::Good, "Alice <alice@example.com>")
        );
        assert_eq!(
            parse(b"B\0Bob\n"),
            signature(CommitSignatureStatus::Bad, "Bob")
        );
        assert_eq!(parse(b"U\0"), signature(CommitSignatureStatus::Unknown, ""));
        assert_eq!(parse(b"N\n"), signature(CommitSignatureStatus::None, ""));
        assert_eq!(
            parse(b"X\0Cara\n"),
            signature(CommitSignatureStatus::Expired, "Cara")
        );
        assert_eq!(
            parse(b"Y\0Dave\n"),
            signature(CommitSignatureStatus::GoodExpired, "Dave")
        );
        assert_eq!(
            parse(b"R\0Erin\n"),
            signature(CommitSignatureStatus::Revoked, "Erin")
        );
        assert_eq!(parse(b"E\n"), signature(CommitSignatureStatus::Error, ""));
        assert_eq!(
            parse(b"Q\0odd\n"),
            signature(CommitSignatureStatus::Other("Q".into()), "odd")
        );
    }

    #[test]
    fn version_probe_times_out_without_running_a_mutation() {
        let path =
            std::env::temp_dir().join(format!("gitronimo-slow-version-{}", std::process::id()));
        fs::write(&path, "#!/bin/sh\nsleep 1\n").expect("probe fixture should write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("probe fixture should be executable");
        let error = GitExecutable(path.clone())
            .version_with_timeout(Duration::ZERO)
            .expect_err("stalled version probe should time out");
        let _ = fs::remove_file(path);
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    fn temporary_commit_messages() -> Vec<std::path::PathBuf> {
        let prefix = format!("gitronimo-commit-{}-", std::process::id());
        let mut paths = fs::read_dir(std::env::temp_dir())
            .expect("temporary directory should read")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

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

        fn at(path: std::path::PathBuf) -> Self {
            let git =
                GitExecutable::discover().expect("Git should be installed for integration tests");
            let repository = Self { path, git };
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

    fn diff_text(diff: &git_domain::UnifiedDiff) -> String {
        diff.files
            .iter()
            .flat_map(|file| file.hunks.iter())
            .flat_map(|hunk| hunk.lines.iter())
            .map(|line| String::from_utf8_lossy(&line.content).into_owned())
            .collect::<Vec<_>>()
            .join("\n")
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

    fn rev_parse(repository: &Repository, rev: &str) -> String {
        String::from_utf8(
            repository
                .git
                .run(&repository.path, ["rev-parse", rev])
                .expect("rev-parse should run")
                .stdout,
        )
        .expect("oid utf-8")
        .trim()
        .to_owned()
    }

    fn worktree_of(repository: &Repository) -> git_domain::WorktreeRepository {
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        worktree
    }

    #[test]
    fn checkout_detached_leaves_head_detached() {
        let repository = Repository::new();
        repository.commit("first");
        repository.commit("second");
        let worktree = worktree_of(&repository);
        let first_oid = rev_parse(&repository, "HEAD~1");
        repository
            .git
            .checkout_detached(&worktree, &first_oid)
            .expect("detach onto first commit");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status after detach");
        assert_eq!(status.branch.head, HeadStatus::Detached);
    }

    #[test]
    fn reset_to_supports_soft_mixed_and_hard() {
        let repository = Repository::new();
        repository.commit("first");
        repository.commit("second");
        let worktree = worktree_of(&repository);
        let first_oid = rev_parse(&repository, "HEAD~1");
        repository
            .git
            .reset_to(&worktree, &first_oid, ResetMode::Soft)
            .expect("soft reset");
        let soft = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status after soft reset");
        assert!(!soft.entries.is_empty(), "soft reset keeps later change");
        assert_eq!(rev_parse(&repository, "HEAD"), first_oid);

        repository.success(["reset", "--hard", "HEAD"]);
        repository.commit("third");
        let before = rev_parse(&repository, "HEAD~1");
        repository
            .git
            .reset_to(&worktree, &before, ResetMode::Mixed)
            .expect("mixed reset");
        repository
            .git
            .reset_to(&worktree, &before, ResetMode::Hard)
            .expect("hard reset");
        assert_eq!(rev_parse(&repository, "HEAD"), before);
    }

    #[test]
    fn format_patch_to_dir_writes_one_patch() {
        let repository = Repository::new();
        repository.commit("first");
        let worktree = worktree_of(&repository);
        let tip = rev_parse(&repository, "HEAD");
        let patch_dir = repository.path.join("patches");
        fs::create_dir(&patch_dir).expect("patch dir");
        repository
            .git
            .format_patch_to_dir(&worktree, &tip, &patch_dir)
            .expect("format-patch writes one file");
        let patches: Vec<_> = fs::read_dir(&patch_dir)
            .expect("patch dir reads")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(patches.len(), 1, "exactly one patch file");
        assert!(
            patches[0]
                .path()
                .extension()
                .is_some_and(|ext| ext == "patch"),
            "format-patch emits a .patch file"
        );
    }

    #[test]
    fn initializes_a_new_repository_in_a_selected_directory() {
        let repository = Repository::new();
        let created = repository.path.join("new-project");
        fs::create_dir(&created).expect("new project directory should exist");
        repository
            .git
            .init_repository(&created)
            .expect("Git should initialize the selected directory");
        let location = repository
            .git
            .discover_repository(&created)
            .expect("initialized directory should be discoverable");
        let RepositoryLocation::Worktree(worktree) = location else {
            panic!("initialized directory should be a worktree");
        };
        assert_eq!(
            worktree.worktree_root,
            created
                .canonicalize()
                .expect("created path should canonicalize")
        );
    }

    #[test]
    fn clones_a_local_repository_into_a_typed_destination() {
        let repository = Repository::new();
        repository.commit("initial");
        let remote = repository.path.with_extension("clone.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let destination = repository.path.with_extension("cloned");
        repository
            .git
            .clone_repository(
                remote.to_str().expect("temporary path is UTF-8"),
                &destination,
            )
            .expect("local clone should complete");
        assert_eq!(
            fs::read_to_string(destination.join("fixture.txt")).expect("cloned file should exist"),
            "initial"
        );
        let _ = fs::remove_dir_all(remote);
        let _ = fs::remove_dir_all(destination);
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
    fn records_old_and_new_line_numbers_for_diff_lines() {
        let diff = parse_unified_diff(
            b"diff --git a/fixture.txt b/fixture.txt\n\
index 1111111..2222222 100644\n\
--- a/fixture.txt\n\
+++ b/fixture.txt\n\
@@ -2,4 +3,5 @@ second\n\
\x20third\n\
-removed\n\
+added\n\
\x20fifth\n\
\x20sixth\n",
        );
        let hunk = &diff.files[0].hunks[0];
        assert_eq!(hunk.lines[0].kind, DiffLineKind::Context);
        assert_eq!(hunk.lines[0].old_line, Some(2));
        assert_eq!(hunk.lines[0].new_line, Some(3));
        assert_eq!(hunk.lines[1].kind, DiffLineKind::Removal);
        assert_eq!(hunk.lines[1].old_line, Some(3));
        assert_eq!(hunk.lines[1].new_line, None);
        assert_eq!(hunk.lines[2].kind, DiffLineKind::Addition);
        assert_eq!(hunk.lines[2].old_line, None);
        assert_eq!(hunk.lines[2].new_line, Some(4));
        assert_eq!(hunk.lines[3].kind, DiffLineKind::Context);
        assert_eq!(hunk.lines[3].old_line, Some(4));
        assert_eq!(hunk.lines[3].new_line, Some(5));
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
    fn loads_staged_and_unstaged_file_diffs() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "staged").expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let staged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), true)
            .expect("staged diff should load");
        assert_eq!(staged.diff.files.len(), 1);
        fs::write(repository.path.join("fixture.txt"), "unstaged").expect("fixture should write");
        let unstaged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), false)
            .expect("unstaged diff should load");
        assert_eq!(unstaged.diff.files.len(), 1);
    }

    #[test]
    fn stages_and_unstages_paths_and_all_changes() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("one.txt"), "one").expect("fixture should write");
        fs::write(repository.path.join("two.txt"), "two").expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let paths = [GitPath(b"one.txt".to_vec()), GitPath(b"two.txt".to_vec())];
        repository
            .git
            .stage_paths(&worktree, &paths)
            .expect("paths should stage");
        assert_eq!(
            repository
                .git
                .worktree_status(&worktree, false)
                .expect("status should load")
                .entries
                .len(),
            2
        );
        repository
            .git
            .unstage_paths(&worktree, &paths)
            .expect("paths should unstage");
        repository
            .git
            .stage_all(&worktree)
            .expect("all should stage");
        repository
            .git
            .unstage_all(&worktree)
            .expect("all should unstage");
    }

    #[test]
    fn stages_only_the_requested_unstaged_hunk() {
        let repository = Repository::new();
        fs::write(
            repository.path.join("fixture.txt"),
            "first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "first changed\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth changed\n",
        )
        .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        repository
            .git
            .stage_hunk(&worktree, &GitPath(b"fixture.txt".to_vec()), 0)
            .expect("first hunk should stage");

        let staged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), true)
            .expect("staged diff should load");
        let unstaged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), false)
            .expect("unstaged diff should load");
        let staged_text = diff_text(&staged.diff);
        let unstaged_text = diff_text(&unstaged.diff);
        assert!(staged_text.contains("first changed"));
        assert!(!staged_text.contains("tenth changed"));
        assert!(!unstaged_text.contains("first changed"));
        assert!(unstaged_text.contains("tenth changed"));

        repository
            .git
            .unstage_hunk(&worktree, &GitPath(b"fixture.txt".to_vec()), 0)
            .expect("first staged hunk should unstage");
        let staged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), true)
            .expect("staged diff should load");
        let unstaged = repository
            .git
            .file_diff(&worktree, &GitPath(b"fixture.txt".to_vec()), false)
            .expect("unstaged diff should load");
        assert!(staged.diff.files.is_empty());
        let unstaged_text = diff_text(&unstaged.diff);
        assert!(unstaged_text.contains("first changed"));
        assert!(unstaged_text.contains("tenth changed"));
    }

    #[test]
    fn discards_only_the_requested_unstaged_hunk() {
        let repository = Repository::new();
        fs::write(
            repository.path.join("fixture.txt"),
            "first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "first changed\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth changed\n",
        )
        .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = GitPath(b"fixture.txt".to_vec());

        repository
            .git
            .discard_hunk(&worktree, &path, 0)
            .expect("first hunk should discard");

        let remaining =
            fs::read_to_string(repository.path.join("fixture.txt")).expect("fixture should read");
        assert!(!remaining.contains("first changed"));
        assert!(remaining.contains("tenth changed"));
        assert!(remaining.starts_with("first\n"));
        let staged = repository
            .git
            .file_diff(&worktree, &path, true)
            .expect("staged diff should load");
        assert!(
            staged.diff.files.is_empty(),
            "the index must stay untouched"
        );
    }

    fn selection_for(diff: &git_domain::UnifiedDiff, content: &[u8]) -> (usize, usize) {
        diff.files[0]
            .hunks
            .iter()
            .enumerate()
            .find_map(|(hunk_index, hunk)| {
                hunk.lines
                    .iter()
                    .position(|line| line.kind != DiffLineKind::Context && line.content == content)
                    .map(|line_index| (hunk_index, line_index))
            })
            .expect("fixture should expose the requested change line")
    }

    #[test]
    fn stages_only_the_requested_unstaged_lines() {
        let repository = Repository::new();
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\niota\nkappa\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\nINSERT ONE\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\nINSERT TWO\niota\nkappa\n",
        )
        .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = GitPath(b"fixture.txt".to_vec());
        let unstaged = repository
            .git
            .file_diff(&worktree, &path, false)
            .expect("unstaged diff should load");
        let selection = selection_for(&unstaged.diff, b"INSERT ONE");

        repository
            .git
            .stage_lines(&worktree, &path, std::slice::from_ref(&selection))
            .expect("selected addition should stage");

        let staged = repository
            .git
            .file_diff(&worktree, &path, true)
            .expect("staged diff should load");
        let unstaged = repository
            .git
            .file_diff(&worktree, &path, false)
            .expect("unstaged diff should load");
        let staged_text = diff_text(&staged.diff);
        let unstaged_text = diff_text(&unstaged.diff);
        assert!(staged_text.contains("INSERT ONE"));
        assert!(!staged_text.contains("INSERT TWO"));
        assert!(!unstaged_text.contains("INSERT ONE"));
        assert!(unstaged_text.contains("INSERT TWO"));
    }

    #[test]
    fn discards_only_the_requested_unstaged_lines() {
        let repository = Repository::new();
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\niota\nkappa\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\nINSERT ONE\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\nINSERT TWO\niota\nkappa\n",
        )
        .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = GitPath(b"fixture.txt".to_vec());
        let unstaged = repository
            .git
            .file_diff(&worktree, &path, false)
            .expect("unstaged diff should load");
        let selection = selection_for(&unstaged.diff, b"INSERT ONE");

        repository
            .git
            .discard_lines(&worktree, &path, std::slice::from_ref(&selection))
            .expect("selected addition should discard");

        let remaining =
            fs::read_to_string(repository.path.join("fixture.txt")).expect("fixture should read");
        assert!(!remaining.contains("INSERT ONE"));
        assert!(remaining.contains("INSERT TWO"));
        assert!(remaining.starts_with("alpha\nbeta\n"));
        assert!(remaining.ends_with("iota\nkappa\n"));
    }

    #[test]
    fn discarding_a_selected_removal_restores_the_deleted_line() {
        let repository = Repository::new();
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\ngamma\ndelta\nepsilon\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "alpha\nbeta\ndelta\nepsilon\n",
        )
        .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = GitPath(b"fixture.txt".to_vec());
        let unstaged = repository
            .git
            .file_diff(&worktree, &path, false)
            .expect("unstaged diff should load");
        let selection = selection_for(&unstaged.diff, b"gamma");

        repository
            .git
            .discard_lines(&worktree, &path, std::slice::from_ref(&selection))
            .expect("selected removal should discard");

        let remaining =
            fs::read_to_string(repository.path.join("fixture.txt")).expect("fixture should read");
        assert_eq!(remaining, "alpha\nbeta\ngamma\ndelta\nepsilon\n");
    }

    #[test]
    fn line_selection_rejects_out_of_range_indexes() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\n").expect("fixture write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\nADDED\n")
            .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = GitPath(b"fixture.txt".to_vec());
        let error = repository
            .git
            .stage_lines(&worktree, &path, &[(0, 99)])
            .expect_err("out-of-range selection should fail");
        assert!(matches!(error, GitStatusError::PatchLinesUnavailable));
    }

    #[test]
    fn stashes_tracked_changes_and_optionally_untracked_files() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "tracked change")
            .expect("fixture should write");
        fs::write(repository.path.join("untracked.txt"), "untracked change")
            .expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        repository
            .git
            .create_stash(&worktree, false)
            .expect("tracked changes should stash");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 1);
        assert!(status.entries.iter().any(
            |entry| matches!(entry, StatusEntry::Untracked(path) if path.0 == b"untracked.txt")
        ));
        repository
            .git
            .pop_latest_stash(&worktree)
            .expect("latest stash should pop");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 0);
        repository
            .git
            .create_stash(&worktree, true)
            .expect("untracked changes should stash when requested");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 1);
        assert!(status.entries.is_empty());
        repository
            .git
            .apply_latest_stash(&worktree)
            .expect("latest stash should apply");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 1);
        assert!(status.entries.iter().any(
            |entry| matches!(entry, StatusEntry::Untracked(path) if path.0 == b"untracked.txt")
        ));
        fs::write(repository.path.join("fixture.txt"), "second change")
            .expect("fixture should write");
        repository
            .git
            .create_stash(&worktree, true)
            .expect("fixture should stash");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 2);
        repository
            .git
            .drop_latest_stash(&worktree)
            .expect("latest stash should drop");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 1);
    }

    #[test]
    fn lists_stashes_and_applies_pops_drops_by_reference() {
        let repository = Repository::new();
        repository.commit("initial");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        fs::write(repository.path.join("fixture.txt"), "first change")
            .expect("fixture should write");
        repository
            .git
            .create_stash(&worktree, false)
            .expect("first change should stash");
        fs::write(repository.path.join("fixture.txt"), "second change")
            .expect("fixture should write");
        repository
            .git
            .create_stash(&worktree, false)
            .expect("second change should stash");
        let stashes = repository
            .git
            .stash_list(&worktree)
            .expect("stash list should load");
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].reference, "stash@{0}");
        assert_eq!(stashes[1].reference, "stash@{1}");
        assert!(!stashes[0].subject.is_empty());
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 2);
        repository
            .git
            .apply_stash(&worktree, &stashes[1].reference)
            .expect("older stash should apply");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 2);
        assert_eq!(
            fs::read_to_string(repository.path.join("fixture.txt")).expect("file should exist"),
            "first change"
        );
        fs::write(repository.path.join("fixture.txt"), "initial").expect("fixture should write");
        repository
            .git
            .drop_stash(&worktree, &stashes[1].reference)
            .expect("older stash should drop");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 1);
        repository
            .git
            .pop_stash(&worktree, &stashes[0].reference)
            .expect("newest stash should pop");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(status.stash_count, 0);
        assert_eq!(
            fs::read_to_string(repository.path.join("fixture.txt")).expect("file should exist"),
            "second change"
        );
    }

    #[test]
    fn reports_modified_lfs_files_in_a_temporary_repository() {
        let repository = Repository::new();
        let probe = repository
            .git
            .run(&repository.path, ["lfs", "version"])
            .expect("Git LFS probe should run");
        if !probe.status.success() {
            return;
        }
        repository.success(["lfs", "track", "*.bin"]);
        fs::write(repository.path.join("large.bin"), b"initial LFS content")
            .expect("LFS file should write");
        repository.success(["add", ".gitattributes", "large.bin"]);
        repository.success(["commit", "-m", "add LFS file"]);
        fs::write(repository.path.join("large.bin"), b"changed LFS content")
            .expect("LFS file should change");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let entries = repository
            .git
            .lfs_status(&worktree)
            .expect("LFS status should load");
        let entry = entries
            .iter()
            .find(|entry| entry.path.0 == b"large.bin")
            .expect("modified LFS file should be listed");
        assert_eq!(entry.index_status, b' ');
        assert_eq!(entry.worktree_status, b'M');
    }

    #[test]
    fn commits_amends_signs_off_and_preserves_failure_path() {
        let repository = Repository::new();
        let messages_before = temporary_commit_messages();
        fs::write(repository.path.join("fixture.txt"), "first").expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        repository
            .git
            .stage_all(&worktree)
            .expect("fixture should stage");
        repository
            .git
            .commit(
                &worktree,
                &CommitRequest {
                    subject: "first".into(),
                    body: "body".into(),
                    amend: false,
                    sign_off: true,
                },
            )
            .expect("commit should succeed");
        let log = repository
            .git
            .run(&repository.path, ["log", "-1", "--format=%B"])
            .expect("log should run");
        assert!(String::from_utf8_lossy(&log.stdout).contains("Signed-off-by:"));
        fs::write(repository.path.join("fixture.txt"), "amended").expect("fixture should write");
        repository
            .git
            .stage_all(&worktree)
            .expect("fixture should stage");
        repository
            .git
            .commit(
                &worktree,
                &CommitRequest {
                    subject: "amended".into(),
                    body: String::new(),
                    amend: true,
                    sign_off: false,
                },
            )
            .expect("amend should succeed");
        let summary = repository
            .git
            .head_commit_summary(&worktree)
            .expect("head summary should load");
        assert_eq!(summary.subject, "amended");
        assert!(
            summary.short_oid.len() >= 7,
            "short oid should be abbreviated"
        );
        let hook = repository.path.join(".git/hooks/pre-commit");
        fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("hook should write");
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
            .expect("hook should be executable");
        fs::write(repository.path.join("fixture.txt"), "rejected").expect("fixture should write");
        repository
            .git
            .stage_all(&worktree)
            .expect("fixture should stage");
        assert!(matches!(
            repository.git.commit(
                &worktree,
                &CommitRequest {
                    subject: "rejected".into(),
                    body: String::new(),
                    amend: false,
                    sign_off: false
                }
            ),
            Err(GitStatusError::CommandFailed(message)) if !message.is_empty()
        ));
        assert_eq!(temporary_commit_messages(), messages_before);
    }

    #[test]
    fn discards_tracked_paths_and_refuses_untracked_or_unsafe_ones() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "changed").expect("fixture should write");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        repository
            .git
            .discard_tracked_paths(&worktree, &[GitPath(b"fixture.txt".to_vec())])
            .expect("tracked path should restore");
        assert_eq!(
            fs::read_to_string(repository.path.join("fixture.txt")).expect("fixture should read"),
            "initial"
        );
        fs::write(repository.path.join("untracked.txt"), "keep").expect("fixture should write");
        assert!(matches!(
            repository
                .git
                .discard_tracked_paths(&worktree, &[GitPath(b"untracked.txt".to_vec())]),
            Err(GitStatusError::UntrackedDeletionRefused)
        ));
        assert!(matches!(
            repository
                .git
                .discard_tracked_paths(&worktree, &[GitPath(b"../outside".to_vec())]),
            Err(GitStatusError::UnsafePath)
        ));
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
    fn loads_bounded_history_pages_and_separate_decorations() {
        let repository = Repository::new();
        for index in 0..3 {
            repository.commit(&format!("commit {index}"));
        }
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        let first = repository
            .git
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: None,
                    limit: 2,
                },
            )
            .expect("first page should load");
        assert_eq!(first.commits.len(), 2);
        let selected = &first.commits[0].oid;
        assert_eq!(
            repository
                .git
                .commit_paths(&worktree, selected)
                .expect("commit paths should load"),
            vec![GitPath(b"fixture.txt".to_vec())]
        );
        assert_eq!(
            repository
                .git
                .commit_diff(&worktree, selected)
                .expect("commit diff should load")
                .diff
                .files
                .len(),
            1
        );
        assert!(
            first
                .commits
                .iter()
                .all(|commit| !commit.author.name.is_empty() && !commit.subject.is_empty())
        );
        let second = repository
            .git
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: first.next_before,
                    limit: 2,
                },
            )
            .expect("next page should load");
        assert!(!second.commits.is_empty());
        assert!(
            repository
                .git
                .ref_decorations(&worktree)
                .expect("ref decorations should load")
                .iter()
                .any(|decoration| decoration.name == b"main")
        );
    }

    #[test]
    fn pages_all_refs_and_named_history() {
        let repository = Repository::new();
        for index in 0..3 {
            repository.commit(&format!("commit {index}"));
        }
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        let first = repository
            .git
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::All,
                    before: None,
                    limit: 2,
                },
            )
            .expect("all refs history should load");
        assert_eq!(first.commits.len(), 2);
        assert!(
            repository
                .git
                .history_page(
                    &worktree,
                    &HistoryRequest {
                        reference: HistoryReference::All,
                        before: first.next_before,
                        limit: 2,
                    },
                )
                .expect("all refs next page should load")
                .commits
                .iter()
                .all(|commit| !commit.oid.is_empty())
        );
        assert!(
            repository
                .git
                .history_page(
                    &worktree,
                    &HistoryRequest {
                        reference: HistoryReference::Named("main".into()),
                        before: None,
                        limit: 2,
                    },
                )
                .expect("named history should load")
                .commits
                .iter()
                .all(|commit| !commit.oid.is_empty())
        );
    }

    #[test]
    fn loads_ref_snapshot_from_a_real_repository() {
        let repository = Repository::new();
        repository.commit("initial");
        repository.success(["branch", "feature/nested"]);
        repository.success(["tag", "v1.0.0"]);
        let remote = repository.path.with_extension("refs.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success(["fetch", "origin"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        let snapshot = repository
            .git
            .ref_snapshot(&worktree)
            .expect("snapshot should load");
        assert!(
            snapshot
                .local_branches
                .iter()
                .any(|branch| branch.name.0 == b"main")
        );
        assert!(
            snapshot
                .local_branches
                .iter()
                .any(|branch| branch.name.0 == b"feature/nested")
        );
        assert!(
            snapshot
                .remote_branches
                .iter()
                .any(|branch| branch.name.0 == b"origin/main")
        );
        assert!(snapshot.tags.iter().any(|tag| tag.name.0 == b"v1.0.0"));
        assert!(
            snapshot
                .remotes
                .iter()
                .any(|remote| remote.name.0 == b"origin")
        );
        let _ = fs::remove_dir_all(remote);
    }

    #[test]
    fn creates_switches_renames_and_safely_deletes_branches() {
        let repository = Repository::new();
        repository.commit("initial");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        repository
            .git
            .create_branch(&worktree, "feature/from-head", None)
            .expect("branch should create from HEAD");
        repository
            .git
            .rename_branch(&worktree, "feature/from-head", "feature/renamed")
            .expect("branch should rename");
        repository
            .git
            .checkout_branch(&worktree, "main")
            .expect("main should checkout");
        repository
            .git
            .delete_branch(&worktree, "feature/renamed", false)
            .expect("merged branch should delete");
        repository
            .git
            .create_branch(&worktree, "topic", Some("main"))
            .expect("branch should create from explicit ref");
        repository.commit("unmerged");
        repository
            .git
            .checkout_branch(&worktree, "main")
            .expect("main should checkout");
        assert!(
            repository
                .git
                .delete_branch(&worktree, "topic", false)
                .is_err()
        );
        repository
            .git
            .delete_branch(&worktree, "topic", true)
            .expect("explicit forced deletion should work");
    }

    #[test]
    fn checks_out_a_remote_tracking_branch() {
        let repository = Repository::new();
        repository.commit("initial");
        repository.success(["branch", "topic"]);
        let remote = repository.path.with_extension("track.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let clone_path = repository.path.with_extension("track-clone");
        repository.success([
            "clone",
            remote.to_str().expect("temporary path is UTF-8"),
            clone_path.to_str().expect("temporary path is UTF-8"),
        ]);
        let clone = Repository::at(clone_path);
        let RepositoryLocation::Worktree(worktree) = clone
            .git
            .discover_repository(&clone.path)
            .expect("clone should resolve")
        else {
            panic!("clone should be worktree")
        };
        clone
            .git
            .checkout_branch(&worktree, "main")
            .expect("main should checkout");
        clone
            .git
            .checkout_tracking_branch(&worktree, "origin/topic")
            .expect("tracking checkout should create local topic");
        let snapshot = clone
            .git
            .ref_snapshot(&worktree)
            .expect("snapshot should load");
        assert!(
            snapshot
                .local_branches
                .iter()
                .any(|branch| branch.name.0 == b"topic")
        );
        let status = clone
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert_eq!(
            status.branch.head,
            HeadStatus::Branch(GitPath(b"topic".to_vec()))
        );
        let _ = fs::remove_dir_all(remote);
    }

    #[test]
    fn creates_and_deletes_tags_and_exports_an_archive() {
        let repository = Repository::new();
        repository.commit("initial");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        repository
            .git
            .create_tag(&worktree, "v0.1.0", "HEAD")
            .expect("tag should create");
        let archive = repository.path.join("v0.1.0.zip");
        repository
            .git
            .export_archive(&worktree, "v0.1.0", &archive)
            .expect("archive should write");
        assert!(archive.is_file());
        repository
            .git
            .delete_tag(&worktree, "v0.1.0")
            .expect("tag should delete");
        let snapshot = repository
            .git
            .ref_snapshot(&worktree)
            .expect("refs should load");
        assert!(!snapshot.tags.iter().any(|tag| tag.name.0 == b"v0.1.0"));
    }

    #[test]
    fn sets_and_unsets_a_branch_upstream() {
        let repository = Repository::new();
        repository.commit("initial");
        let remote = repository.path.with_extension("upstream.git");
        repository.success([
            "clone",
            "--bare",
            repository.path.to_str().unwrap(),
            remote.to_str().unwrap(),
        ]);
        repository.success(["remote", "add", "origin", remote.to_str().unwrap()]);
        repository.success(["fetch", "origin"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        repository
            .git
            .set_branch_upstream(&worktree, "main", "origin/main")
            .expect("upstream should set");
        let snapshot = repository
            .git
            .ref_snapshot(&worktree)
            .expect("refs should load");
        let main = snapshot
            .local_branches
            .iter()
            .find(|branch| branch.name.0 == b"main")
            .expect("main exists");
        assert_eq!(main.upstream.as_deref(), Some("origin/main"));
        repository
            .git
            .unset_branch_upstream(&worktree, "main")
            .expect("upstream should clear");
        let snapshot = repository
            .git
            .ref_snapshot(&worktree)
            .expect("refs should reload");
        let main = snapshot
            .local_branches
            .iter()
            .find(|branch| branch.name.0 == b"main")
            .expect("main exists");
        assert!(main.upstream.is_none());
        let _ = fs::remove_dir_all(remote);
    }

    #[test]
    fn fetches_pushes_and_publishes_to_a_local_bare_remote() {
        let repository = Repository::new();
        repository.commit("initial");
        let remote = repository.path.with_extension("publish.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        repository
            .git
            .fetch_remote(&worktree, "origin")
            .expect("fetch should work");
        repository
            .git
            .publish_branch(&worktree, "origin", "main")
            .expect("publish should set upstream");
        repository.commit("push");
        repository
            .git
            .push_current(&worktree)
            .expect("ordinary push should work");
        let collaborator = repository.path.with_extension("pull-collaborator");
        repository.success([
            "clone",
            remote.to_str().expect("temporary path is UTF-8"),
            collaborator.to_str().expect("temporary path is UTF-8"),
        ]);
        let collaborator_repository = Repository::at(collaborator.clone());
        collaborator_repository.commit("remote change");
        collaborator_repository.success(["push"]);
        repository
            .git
            .pull_current(&worktree)
            .expect("pull should apply the configured upstream");
        let _ = fs::remove_dir_all(remote);
        let _ = fs::remove_dir_all(collaborator);
    }

    #[test]
    fn fetches_a_pull_request_ref_into_a_remote_tracking_branch() {
        let repository = Repository::new();
        repository.commit("initial");
        let remote = repository.path.with_extension("pull-request.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success([
            "--git-dir",
            remote.to_str().expect("temporary path is UTF-8"),
            "update-ref",
            "refs/pull/7/head",
            "HEAD",
        ]);
        repository.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be a worktree");
        };
        repository
            .git
            .fetch_pull_request(&worktree, "origin", 7)
            .expect("pull request ref should fetch");
        let fetched = repository
            .git
            .run(&repository.path, ["rev-parse", "refs/remotes/origin/pr/7"])
            .expect("fetched ref should resolve");
        assert!(fetched.status.success());
        let _ = fs::remove_dir_all(remote);
    }

    #[test]
    fn force_with_lease_updates_a_fetched_diverged_branch() {
        let repository = Repository::new();
        repository.commit("initial");
        let remote = repository.path.with_extension("lease.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("worktree should resolve")
        else {
            panic!("fixture should be worktree")
        };
        repository
            .git
            .publish_branch(&worktree, "origin", "main")
            .expect("main should publish");
        let collaborator = repository.path.with_extension("collaborator");
        repository.success([
            "clone",
            remote.to_str().expect("temporary path is UTF-8"),
            collaborator.to_str().expect("temporary path is UTF-8"),
        ]);
        let collaborator_repository = Repository::at(collaborator.clone());
        collaborator_repository.commit("remote change");
        collaborator_repository.success(["push"]);
        repository.commit("local change");
        repository
            .git
            .fetch_remote(&worktree, "origin")
            .expect("tracking ref should refresh");
        repository
            .git
            .push_current_with_lease(&worktree)
            .expect("explicit lease push should update the diverged remote");
        let _ = fs::remove_dir_all(remote);
        let _ = fs::remove_dir_all(collaborator);
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

    #[test]
    fn detects_a_conflicting_merge_and_its_abort() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["branch", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        repository.success(["checkout", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let merged = repository
            .git
            .run(&repository.path, ["merge", "topic"])
            .expect("merge should run");
        assert!(!merged.status.success(), "conflicting merge should fail");
        let operation = repository.git.in_progress_operation(&worktree);
        let InProgressOperation::Merge { oid } = &operation else {
            panic!("merge should be reported in progress: {operation:?}");
        };
        assert!(oid.as_ref().is_some_and(|oid| !oid.is_empty()));

        repository.success(["merge", "--abort"]);
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
    }

    #[test]
    fn detects_a_conflicting_rebase_and_its_abort() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        repository.success(["checkout", "topic"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let rebased = repository
            .git
            .run(&repository.path, ["rebase", "main"])
            .expect("rebase should run");
        assert!(!rebased.status.success(), "conflicting rebase should fail");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::Rebase
        );

        repository.success(["rebase", "--abort"]);
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
    }

    #[test]
    fn detects_conflicting_cherry_picks_and_reverts_and_their_aborts() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let picked = repository
            .git
            .run(&repository.path, ["cherry-pick", "topic"])
            .expect("cherry-pick should run");
        assert!(
            !picked.status.success(),
            "conflicting cherry-pick should fail"
        );
        let operation = repository.git.in_progress_operation(&worktree);
        let InProgressOperation::CherryPick { oid } = &operation else {
            panic!("cherry-pick should be reported in progress: {operation:?}");
        };
        assert!(oid.as_ref().is_some_and(|oid| !oid.is_empty()));

        repository.success(["cherry-pick", "--abort"]);
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );

        let main_version = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "HEAD"])
                .expect("HEAD should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        fs::write(repository.path.join("fixture.txt"), "alpha\nB\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "second main change"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "third main change"]);
        let reverted = repository
            .git
            .run(&repository.path, ["revert", "--no-edit", &main_version])
            .expect("revert should run");
        assert!(!reverted.status.success(), "conflicting revert should fail");
        let operation = repository.git.in_progress_operation(&worktree);
        let InProgressOperation::Revert { oid } = &operation else {
            panic!("revert should be reported in progress: {operation:?}");
        };
        assert!(oid.as_ref().is_some_and(|oid| !oid.is_empty()));

        repository.success(["revert", "--abort"]);
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
    }

    #[test]
    fn merges_clean_and_conflicting_histories_and_aborts() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        repository
            .git
            .merge_branch(&worktree, "topic")
            .expect("fast-forward merge should succeed");
        assert_eq!(
            fs::read_to_string(repository.path.join("fixture.txt")).expect("fixture should read"),
            "alpha\nTOPIC\ngamma\n"
        );

        repository.success(["checkout", "-b", "conflict", "main~1"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        let merged = repository
            .git
            .merge_branch(&worktree, "topic")
            .expect_err("conflicting merge should fail");
        assert!(matches!(
            merged,
            GitStatusError::CommandFailed(ref message) if message.contains("CONFLICT")
        ));
        let operation = repository.git.in_progress_operation(&worktree);
        let InProgressOperation::Merge { oid } = &operation else {
            panic!("merge should be reported in progress: {operation:?}");
        };
        assert!(oid.as_ref().is_some_and(|oid| !oid.is_empty()));
        repository
            .git
            .abort_merge(&worktree)
            .expect("merge should abort");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
    }

    #[test]
    fn resolves_stages_and_continues_a_conflicting_merge() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["branch", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        repository.success(["checkout", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = repository.path.clone();

        repository
            .git
            .merge_branch(&worktree, "topic")
            .expect_err("conflicting merge should fail");
        let operation = repository.git.in_progress_operation(&worktree);
        fs::write(path.join("fixture.txt"), "alpha\nRESOLVED\ngamma\n")
            .expect("resolution should write");
        repository.success(["add", "fixture.txt"]);
        repository
            .git
            .continue_operation(&worktree, &operation)
            .expect("resolved merge should continue");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
        assert_eq!(
            fs::read_to_string(path.join("fixture.txt")).expect("fixture should read"),
            "alpha\nRESOLVED\ngamma\n"
        );
    }

    #[test]
    fn recovery_snapshot_records_pre_operation_refs() {
        let repository = Repository::new();
        repository.commit("base");
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "topic work\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic commit"]);
        repository.success(["checkout", "main"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let head_before = repository
            .git
            .run(&repository.path, ["rev-parse", "HEAD"])
            .expect("HEAD should resolve")
            .stdout;
        let head_before = trim_oid(&head_before).expect("HEAD should have an oid");

        let snapshot = repository
            .git
            .recovery_snapshot(&worktree)
            .expect("snapshot should load");
        assert_eq!(
            snapshot.old_head.as_deref(),
            Some(head_before.as_slice()),
            "snapshot should capture the pre-operation HEAD"
        );
        assert_eq!(
            snapshot.head_name,
            Some(GitPath(b"refs/heads/main".to_vec())),
            "snapshot should name the current branch"
        );
        let main_tip = snapshot
            .branch_tips
            .iter()
            .find(|tip| tip.name == GitPath(b"refs/heads/main".to_vec()))
            .expect("snapshot should include the main tip");
        assert_eq!(main_tip.oid, head_before);
        assert!(
            snapshot
                .branch_tips
                .iter()
                .any(|tip| tip.name == GitPath(b"refs/heads/topic".to_vec()))
        );

        repository
            .git
            .merge_branch(&worktree, "topic")
            .expect("fast-forward merge should succeed");
        let head_after = repository
            .git
            .run(&repository.path, ["rev-parse", "HEAD"])
            .expect("HEAD should resolve")
            .stdout;
        let head_after = trim_oid(&head_after).expect("HEAD should have an oid");
        assert_ne!(
            head_after, head_before,
            "the merge should move HEAD so the snapshot differs from the new state"
        );
        assert_eq!(
            snapshot.old_head.as_deref(),
            Some(head_before.as_slice()),
            "the recorded snapshot must retain the true pre-operation refs"
        );
    }

    #[test]
    fn reflog_reads_newest_first_with_chained_old_oids_and_restores_a_deleted_branch() {
        let repository = Repository::new();
        repository.commit("base");
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "topic work\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic commit"]);
        let topic_oid = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "HEAD"])
                .expect("HEAD should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        repository.success(["checkout", "main"]);
        repository.success(["branch", "-D", "topic"]);

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let entries = repository
            .git
            .reflog(
                &worktree,
                &ReflogRequest {
                    reference: None,
                    limit: 100,
                },
            )
            .expect("reflog should read");
        assert!(
            entries.len() >= 3,
            "the reflog should cover the checkout, commit, and deletion history"
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.new_oid == topic_oid.as_bytes()),
            "the deleted topic tip should appear in the HEAD reflog"
        );
        for pair in entries.windows(2) {
            assert_eq!(
                pair[0].old_oid.as_deref(),
                Some(pair[1].new_oid.as_slice()),
                "each entry should chain its old oid to the newer entry's oid"
            );
        }
        assert!(
            entries.iter().all(|entry| entry.identity.timestamp > 0),
            "every entry should carry a parsed committer timestamp"
        );
        assert!(
            entries.iter().all(|entry| !entry.selector.is_empty()),
            "every entry should carry a selector"
        );

        repository
            .git
            .restore_branch_from_reflog(&worktree, "topic", &topic_oid)
            .expect("deleted branch should restore at its reflog oid");
        let restored = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "topic"])
                .expect("restored branch should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        assert_eq!(
            restored, topic_oid,
            "the restored branch should point at the original topic tip"
        );

        let conflict = repository
            .git
            .restore_branch_from_reflog(&worktree, "topic", &topic_oid)
            .expect_err("restoring an existing branch should be refused");
        let GitStatusError::CommandFailed(message) = conflict else {
            panic!("restore should surface Git's refusal, got {conflict:?}");
        };
        assert!(
            message.contains("exists"),
            "Git should explain the refusal, got: {message}"
        );
    }

    #[test]
    fn file_history_tracks_a_path_newest_first() {
        let repository = Repository::new();
        fs::write(repository.path.join("notes.txt"), "one\n").expect("notes should write");
        repository.success(["add", "notes.txt"]);
        repository.success(["commit", "-m", "add notes"]);
        fs::write(repository.path.join("notes.txt"), "one\ntwo\n").expect("notes should write");
        repository.success(["add", "notes.txt"]);
        repository.success(["commit", "-m", "grow notes"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let history = repository
            .git
            .file_history(
                &worktree,
                &FileHistoryRequest {
                    path: GitPath(b"notes.txt".to_vec()),
                    limit: 10,
                },
            )
            .expect("file history should load");
        assert_eq!(history.len(), 2, "both commits touched the file");
        assert_eq!(
            String::from_utf8_lossy(&history[0].subject),
            "grow notes",
            "history should be newest first"
        );
        assert_eq!(String::from_utf8_lossy(&history[1].subject), "add notes");
        assert!(
            history[0].oid != history[1].oid,
            "each commit should carry its own oid"
        );
        assert!(
            !history[0].author.name.is_empty(),
            "history should carry the author identity"
        );

        let untouched = repository
            .git
            .file_history(
                &worktree,
                &FileHistoryRequest {
                    path: GitPath(b"missing.txt".to_vec()),
                    limit: 10,
                },
            )
            .expect("a missing path yields empty history, not an error");
        assert!(
            untouched.is_empty(),
            "no commits should touch a missing path"
        );
    }

    #[test]
    fn blame_attributes_each_line_to_its_introducing_commit() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "first line\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "initial"]);
        fs::write(
            repository.path.join("fixture.txt"),
            "first line\nsecond line\n",
        )
        .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "extend"]);
        let first_oid = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-list", "-n", "1", "HEAD^"])
                .expect("parent should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let blame = repository
            .git
            .blame(&worktree, &GitPath(b"fixture.txt".to_vec()))
            .expect("blame should load");
        let head_oid = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "HEAD"])
                .expect("HEAD should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        assert_eq!(blame.len(), 2, "one entry per line");
        assert_eq!(
            String::from_utf8_lossy(&blame[0].content),
            "first line",
            "content should carry the line without a trailing newline"
        );
        assert_eq!(
            String::from_utf8_lossy(&blame[0].oid),
            first_oid,
            "the untouched first line should blame the initial commit"
        );
        assert_eq!(
            String::from_utf8_lossy(&blame[1].oid),
            head_oid,
            "the added second line should blame the extending commit"
        );
        assert!(
            !blame[0].author.name.is_empty(),
            "each line should carry the author"
        );
        assert!(
            blame[0].author.timestamp > 0,
            "each line should carry a parsed author timestamp"
        );
    }

    #[test]
    fn compares_refs_browses_the_tree_and_reads_a_file_at_a_revision() {
        let repository = Repository::new();
        fs::create_dir_all(repository.path.join("dir")).expect("dir should create");
        fs::write(repository.path.join("dir/notes.txt"), "alpha\n").expect("notes should write");
        repository.success(["add", "."]);
        repository.success(["commit", "-m", "add notes"]);
        fs::write(repository.path.join("dir/notes.txt"), "alpha\nbeta\n")
            .expect("notes should write");
        repository.success(["add", "."]);
        repository.success(["commit", "-m", "extend notes"]);
        let first_oid = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-list", "-n", "1", "HEAD^"])
                .expect("parent should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let diff = repository
            .git
            .diff_refs(&worktree, &first_oid, "HEAD")
            .expect("ref diff should load");
        assert!(
            diff.diff.files.iter().any(|file| {
                file.old_path
                    .as_ref()
                    .is_some_and(|path| path == &GitPath(b"dir/notes.txt".to_vec()))
            }),
            "the diff should include the changed file"
        );

        let root = repository
            .git
            .tree_entries(&worktree, "HEAD", &GitPath(Vec::new()))
            .expect("tree should list");
        assert!(
            root.iter()
                .any(|entry| entry.kind == TreeEntryKind::Tree
                    && entry.name == GitPath(b"dir".to_vec())),
            "the root should list the directory tree"
        );

        let bytes = repository
            .git
            .file_at_revision(&worktree, "HEAD", &GitPath(b"dir/notes.txt".to_vec()))
            .expect("file at revision should read");
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            "alpha\nbeta\n",
            "HEAD should contain the extended file"
        );
        let earlier = repository
            .git
            .file_at_revision(&worktree, &first_oid, &GitPath(b"dir/notes.txt".to_vec()))
            .expect("earlier revision should read");
        assert_eq!(
            String::from_utf8_lossy(&earlier),
            "alpha\n",
            "the parent revision should contain the shorter file"
        );

        let missing = repository
            .git
            .tree_entries(&worktree, "not-a-real-oid", &GitPath(Vec::new()))
            .expect_err("an unresolvable oid should be refused");
        assert!(matches!(missing, GitStatusError::CommandFailed(_)));
    }

    #[test]
    fn lists_adds_and_removes_linked_worktrees() {
        let repository = Repository::new();
        repository.commit("base");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let linked = repository
            .path
            .parent()
            .expect("temp root should have a parent")
            .join(format!("gitronimo-linked-{}", std::process::id()));

        let before = repository
            .git
            .worktree_list(&worktree)
            .expect("worktree list should load");
        assert_eq!(before.len(), 1, "a fresh repository has one worktree");
        assert!(before[0].main, "the first worktree is the main one");
        assert!(
            before[0].branch.is_some(),
            "the main worktree is attached to a branch"
        );

        repository
            .git
            .add_worktree(
                &worktree,
                &GitPath(linked.as_os_str().to_string_lossy().as_bytes().to_vec()),
                "linked-branch",
            )
            .expect("linked worktree should add");
        let added = repository
            .git
            .worktree_list(&worktree)
            .expect("worktree list should reload");
        assert_eq!(added.len(), 2, "the linked worktree should appear");
        let linked_entry = added
            .iter()
            .find(|entry| !entry.main)
            .expect("the linked worktree should be listed");
        assert_eq!(
            linked_entry.branch.as_ref().map(|path| path.0.as_slice()),
            Some(b"linked-branch".as_slice()),
            "the linked worktree should track its new branch"
        );
        assert!(
            linked_entry
                .path
                .0
                .windows(8)
                .any(|window| window == b"gitronim"),
            "the linked path should be recorded"
        );

        repository
            .git
            .remove_worktree(
                &worktree,
                &GitPath(linked.as_os_str().to_string_lossy().as_bytes().to_vec()),
                false,
            )
            .expect("linked worktree should remove");
        let after = repository
            .git
            .worktree_list(&worktree)
            .expect("worktree list should reload");
        assert_eq!(
            after.len(),
            1,
            "removal should leave only the main worktree"
        );
        let _ = std::fs::remove_dir_all(&linked);
    }

    #[test]
    fn lists_and_updates_submodules() {
        let source = Repository::new();
        fs::write(source.path.join("lib.txt"), "shared\n").expect("submodule file should write");
        source.success(["add", "lib.txt"]);
        source.success(["commit", "-m", "shared lib"]);
        let source_path = source.path.clone();

        let repository = Repository::new();
        repository.success([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            &source_path.to_string_lossy(),
            "lib",
        ]);
        repository.success(["commit", "-m", "add submodule"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let before = repository
            .git
            .submodule_list(&worktree)
            .expect("submodule list should load");
        assert_eq!(before.len(), 1, "one submodule should be registered");
        assert_eq!(
            before[0].path,
            GitPath(b"lib".to_vec()),
            "the submodule path should parse"
        );
        assert!(
            before[0].flag == b' ' || before[0].flag == b'-',
            "a freshly added submodule is clean or uninitialized, got flag {}",
            before[0].flag as char
        );

        repository
            .git
            .submodule_update(&worktree, Some(&GitPath(b"lib".to_vec())))
            .expect("submodule update should initialize");
        let after = repository
            .git
            .submodule_list(&worktree)
            .expect("submodule list should reload");
        assert!(
            after.iter().all(|entry| entry.flag == b' '),
            "after init every submodule should be clean"
        );
    }

    #[test]
    fn parses_a_rebase_todo_and_round_trips_it() {
        let bytes = b"pick a1b2c3d Feature one\n# a comment to ignore\nfixup e5f6a7b Feature two\nreword 1234567 Yet another\n";
        let items = parse_rebase_todo(bytes);
        assert_eq!(items.len(), 3, "comment lines are skipped");
        assert_eq!(items[0].action, RebaseAction::Pick);
        assert_eq!(items[0].arguments, "a1b2c3d Feature one");
        assert_eq!(items[1].action, RebaseAction::Fixup);
        assert_eq!(items[1].arguments, "e5f6a7b Feature two");
        assert_eq!(items[2].action, RebaseAction::Reword);
        assert_eq!(items[2].arguments, "1234567 Yet another");
    }

    #[test]
    fn edits_the_rebase_plan_mid_conflict_and_continues_the_squash() {
        let repository = Repository::new();
        fs::write(repository.path.join("file.txt"), "base line\n").expect("base file should write");
        repository.success(["add", "file.txt"]);
        repository.success(["commit", "-m", "base"]);

        repository.success(["checkout", "-b", "feature"]);
        fs::write(repository.path.join("file.txt"), "feature line\n")
            .expect("feature file should write");
        repository.success(["add", "file.txt"]);
        repository.success(["commit", "-m", "feature one"]);
        fs::write(repository.path.join("file2.txt"), "extra\n")
            .expect("second feature file should write");
        repository.success(["add", "file2.txt"]);
        repository.success(["commit", "-m", "feature two"]);

        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("file.txt"), "main change\n")
            .expect("main file should write");
        repository.success(["add", "file.txt"]);
        repository.success(["commit", "-m", "main conflict"]);
        repository.success(["checkout", "feature"]);

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let error = repository
            .git
            .rebase_plan(&worktree)
            .expect_err("no rebase should be in progress yet");
        assert!(matches!(error, GitStatusError::NoOperationInProgress));

        repository
            .git
            .start_rebase(&worktree, "main")
            .expect_err("the first patch should conflict");
        let mut plan = repository
            .git
            .rebase_plan(&worktree)
            .expect("the paused rebase should expose its plan");
        assert_eq!(
            plan.len(),
            1,
            "the paused commit is already in git's done list"
        );
        assert_eq!(plan[0].action, RebaseAction::Pick);
        assert!(
            plan[0].arguments.contains("feature two"),
            "the remaining todo replays feature two, got {}",
            plan[0].arguments
        );

        plan[0].action = RebaseAction::Fixup;
        repository
            .git
            .save_rebase_plan(&worktree, &plan)
            .expect("the edited plan should save");

        fs::write(repository.path.join("file.txt"), "resolved\n").expect("conflict should resolve");
        repository.success(["add", "file.txt"]);
        repository
            .git
            .continue_operation(&worktree, &InProgressOperation::Rebase)
            .expect("the rebase should continue to a clean squash");

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should still be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let log = repository
            .git
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: None,
                    limit: 10,
                },
            )
            .expect("history should load");
        assert_eq!(
            log.commits.len(),
            3,
            "base, main conflict, and the squashed feature"
        );
        assert_eq!(
            &log.commits[0].subject[..],
            b"feature one",
            "HEAD is the squashed feature commit"
        );
        assert!(
            repository.path.join("file2.txt").exists(),
            "the fixup's file survives"
        );
    }

    #[test]
    fn squashes_fixups_and_drops_commits() {
        let repository = Repository::new();
        fs::write(repository.path.join("a.txt"), "a\n").expect("a should write");
        repository.success(["add", "a.txt"]);
        repository.success(["commit", "-m", "A"]);
        fs::write(repository.path.join("b.txt"), "b\n").expect("b should write");
        repository.success(["add", "b.txt"]);
        repository.success(["commit", "-m", "B"]);
        fs::write(repository.path.join("c.txt"), "c\n").expect("c should write");
        repository.success(["add", "c.txt"]);
        repository.success(["commit", "-m", "C"]);

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        fs::write(repository.path.join("c.txt"), "fixup\n").expect("fixup change should write");
        repository.success(["add", "c.txt"]);
        repository
            .git
            .autosquash(&worktree, "HEAD", None)
            .expect("fixup should fold into HEAD");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should still be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let history = |repository: &Repository| {
            repository
                .git
                .history_page(
                    &worktree,
                    &HistoryRequest {
                        reference: HistoryReference::Current,
                        before: None,
                        limit: 10,
                    },
                )
                .expect("history should load")
        };
        let log = history(&repository);
        assert_eq!(log.commits.len(), 3, "the fixup folds in place");
        assert_eq!(&log.commits[0].subject[..], b"C", "fixup keeps the subject");
        assert_eq!(
            fs::read_to_string(repository.path.join("c.txt")).expect("c should read"),
            "fixup\n"
        );

        fs::write(repository.path.join("c.txt"), "squash\n").expect("squash change should write");
        repository.success(["add", "c.txt"]);
        repository
            .git
            .autosquash(&worktree, "HEAD", Some("squash note"))
            .expect("squash should fold into HEAD");
        let log = history(&repository);
        assert_eq!(log.commits.len(), 3, "the squash folds in place");
        assert_eq!(
            &log.commits[0].subject[..],
            b"C",
            "squash keeps the subject"
        );
        assert!(
            log.commits[0]
                .body
                .windows(b"squash note".len())
                .any(|w| w == b"squash note"),
            "the squash message is combined into the commit"
        );

        repository
            .git
            .drop_commit(&worktree, "HEAD~1")
            .expect("dropping the middle commit should replay the tip");
        let log = history(&repository);
        assert_eq!(log.commits.len(), 2, "B is dropped from history");
        assert_eq!(&log.commits[0].subject[..], b"C", "the tip survives");
        assert_eq!(&log.commits[1].subject[..], b"A", "the base survives");
        assert!(
            !repository.path.join("b.txt").exists(),
            "dropped commit's file is gone"
        );
        assert!(
            repository.path.join("c.txt").exists(),
            "the surviving commit's file remains"
        );
    }

    #[test]
    fn resolves_a_conflict_to_either_side_and_stages_it() {
        let repository = Repository::new();
        fs::write(repository.path.join("f.txt"), "base\n").expect("base should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "base"]);

        repository.success(["checkout", "-b", "feature"]);
        fs::write(repository.path.join("f.txt"), "theirs\n").expect("feature should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "feature change"]);

        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("f.txt"), "ours\n").expect("main should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "main change"]);

        let merge = repository
            .git
            .run(&repository.path, ["merge", "feature"])
            .expect("git should run");
        assert!(!merge.status.success(), "the merge should conflict");

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert!(
            status
                .entries
                .iter()
                .any(|entry| matches!(entry, StatusEntry::Unmerged { path, .. } if path == &GitPath(b"f.txt".to_vec()))),
            "the merge should leave f.txt conflicted"
        );
        let conflict = repository
            .git
            .read_working_file(&worktree, &GitPath(b"f.txt".to_vec()))
            .expect("the conflicted file should read");
        assert!(
            conflict
                .windows(b"<<<<<<<".len())
                .any(|window| window == b"<<<<<<<"),
            "the working copy shows conflict markers"
        );

        repository
            .git
            .resolve_conflict(&worktree, &GitPath(b"f.txt".to_vec()), ConflictSide::Ours)
            .expect("taking ours should resolve");
        assert_eq!(
            repository
                .git
                .read_working_file(&worktree, &GitPath(b"f.txt".to_vec()))
                .expect("resolved file should read"),
            b"ours\n",
            "taking ours keeps the current branch version"
        );
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should reload");
        assert!(
            !status
                .entries
                .iter()
                .any(|entry| matches!(entry, StatusEntry::Unmerged { .. })),
            "staging marks the conflict resolved"
        );

        repository
            .git
            .continue_operation(
                &worktree,
                &InProgressOperation::Merge {
                    oid: Some(b"feature".to_vec()),
                },
            )
            .expect("the merge should complete");
        assert_eq!(
            repository
                .git
                .read_working_file(&worktree, &GitPath(b"f.txt".to_vec()))
                .expect("merged file should read"),
            b"ours\n"
        );
    }

    #[test]
    fn configures_and_runs_an_external_merge_tool() {
        let repository = Repository::new();
        fs::write(repository.path.join("f.txt"), "base\n").expect("base should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "base"]);

        repository.success(["checkout", "-b", "feature"]);
        fs::write(repository.path.join("f.txt"), "theirs\n").expect("feature should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "feature change"]);

        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("f.txt"), "ours\n").expect("main should write");
        repository.success(["add", "f.txt"]);
        repository.success(["commit", "-m", "main change"]);

        let merge = repository
            .git
            .run(&repository.path, ["merge", "feature"])
            .expect("git should run");
        assert!(!merge.status.success(), "the merge should conflict");

        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        repository
            .git
            .set_merge_tool(&worktree, "noop")
            .expect("the merge tool should persist");
        repository.success(["config", "mergetool.noop.cmd", "true"]);
        repository.success(["config", "mergetool.noop.trustExitCode", "true"]);
        let configured = repository
            .git
            .run(&repository.path, ["config", "merge.tool"])
            .expect("config should read");
        assert_eq!(configured.stdout, b"noop\n", "merge.tool is persisted");

        repository
            .git
            .run_merge_tool(&worktree, None, Some(&GitPath(b"f.txt".to_vec())))
            .expect("the merge tool should run");
        let status = repository
            .git
            .worktree_status(&worktree, false)
            .expect("status should load");
        assert!(
            !status
                .entries
                .iter()
                .any(|entry| matches!(entry, StatusEntry::Unmerged { .. })),
            "a trusted tool resolves the conflict"
        );
        assert!(
            !repository.path.join("f.txt.orig").exists(),
            "keepBackup is disabled"
        );
    }

    #[test]
    fn cherry_picks_reverts_and_continues_resolved_conflicts() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        let topic_oid = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "HEAD"])
                .expect("HEAD should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = repository.path.clone();

        repository
            .git
            .cherry_pick(&worktree, &topic_oid)
            .expect_err("conflicting cherry-pick should fail");
        let picked = repository.git.in_progress_operation(&worktree);
        fs::write(path.join("fixture.txt"), "alpha\nRESOLVED\ngamma\n")
            .expect("resolution should write");
        repository.success(["add", "fixture.txt"]);
        repository
            .git
            .continue_operation(&worktree, &picked)
            .expect("resolved cherry-pick should continue");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );

        let main_version = String::from_utf8_lossy(
            &repository
                .git
                .run(&repository.path, ["rev-parse", "HEAD"])
                .expect("HEAD should resolve")
                .stdout,
        )
        .trim()
        .to_owned();
        fs::write(path.join("fixture.txt"), "alpha\nB\ngamma\n").expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "second main change"]);
        fs::write(path.join("fixture.txt"), "alpha\nC\ngamma\n").expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "third main change"]);
        repository
            .git
            .revert_commit(&worktree, &main_version)
            .expect_err("conflicting revert should fail");
        let reverted = repository.git.in_progress_operation(&worktree);
        fs::write(path.join("fixture.txt"), "alpha\nRESOLVED\ngamma\n")
            .expect("resolution should write");
        repository.success(["add", "fixture.txt"]);
        repository
            .git
            .continue_operation(&worktree, &reverted)
            .expect("resolved revert should continue");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
    }

    #[test]
    fn rebases_and_continues_a_resolved_conflict() {
        let repository = Repository::new();
        fs::write(repository.path.join("fixture.txt"), "alpha\nbeta\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "base"]);
        repository.success(["checkout", "-b", "topic"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nTOPIC\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "topic change"]);
        repository.success(["checkout", "main"]);
        fs::write(repository.path.join("fixture.txt"), "alpha\nMAIN\ngamma\n")
            .expect("fixture should write");
        repository.success(["add", "fixture.txt"]);
        repository.success(["commit", "-m", "main change"]);
        repository.success(["checkout", "topic"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let path = repository.path.clone();

        repository
            .git
            .rebase_branch(&worktree, "main")
            .expect_err("conflicting rebase should fail");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::Rebase
        );
        fs::write(path.join("fixture.txt"), "alpha\nRESOLVED\ngamma\n")
            .expect("resolution should write");
        repository.success(["add", "fixture.txt"]);
        repository
            .git
            .continue_operation(&worktree, &InProgressOperation::Rebase)
            .expect("resolved rebase should continue");
        assert_eq!(
            repository.git.in_progress_operation(&worktree),
            InProgressOperation::None
        );
        assert_eq!(
            fs::read_to_string(path.join("fixture.txt")).expect("fixture should read"),
            "alpha\nRESOLVED\ngamma\n"
        );
    }

    #[test]
    fn continuing_without_an_operation_fails() {
        let repository = Repository::new();
        repository.commit("initial");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };
        let error = repository
            .git
            .continue_operation(&worktree, &InProgressOperation::None)
            .expect_err("continuing nothing should fail");
        assert!(matches!(error, GitStatusError::NoOperationInProgress));
    }

    #[test]
    fn reports_commit_signature_statuses() {
        let repository = Repository::new();
        repository.commit("plain");
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let unsigned = repository
            .git
            .commit_signature(&worktree, "HEAD")
            .expect("signature query should succeed");
        assert_eq!(unsigned.status, CommitSignatureStatus::None);
        assert!(unsigned.signer.is_empty());

        if std::process::Command::new("gpg")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let home = std::env::temp_dir().join(format!(
            "gitronimo-gpg-{}-{}",
            std::process::id(),
            NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&home).expect("gpg home should exist");
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700))
            .expect("gpg home should be private");
        let homedir = home.to_str().expect("temp dir should be unicode");
        let generated = std::process::Command::new("gpg")
            .args([
                "--batch",
                "--homedir",
                homedir,
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--quick-generate-key",
                "Gitronimo Test <test@gitronimo.invalid>",
                "default",
                "default",
                "never",
            ])
            .output();
        if !generated.is_ok_and(|output| output.status.success()) {
            let _ = fs::remove_dir_all(&home);
            return;
        }
        let fingerprints = std::process::Command::new("gpg")
            .args([
                "--batch",
                "--homedir",
                homedir,
                "--with-colons",
                "--list-keys",
            ])
            .output()
            .expect("gpg should list keys");
        let fingerprint = fingerprints
            .stdout
            .split(|byte| *byte == b'\n')
            .find_map(|line| {
                let mut fields = line.split(|byte| *byte == b':');
                (fields.next() == Some(b"fpr")).then(|| fields.nth(8).unwrap_or_default().to_vec())
            })
            .expect("generated key should expose a fingerprint");
        repository.success([
            std::ffi::OsString::from("config"),
            std::ffi::OsString::from("user.signingkey"),
            std::ffi::OsString::from(String::from_utf8_lossy(&fingerprint).into_owned()),
        ]);
        fs::write(repository.path.join("signed.txt"), "signed\n")
            .expect("signed fixture should write");
        repository.success(["add", "signed.txt"]);

        let signed = repository
            .git
            .run_env(
                &repository.path,
                [("GNUPGHOME".into(), home.clone().into_os_string())],
                ["commit", "-S", "-m", "signed"],
            )
            .expect("signed commit should run");
        assert!(signed.status.success(), "signed commit failed: {signed:?}");

        let verified = repository
            .git
            .run_env(
                &repository.path,
                [("GNUPGHOME".into(), home.clone().into_os_string())],
                ["show", "--no-patch", "--format=%G?%x00%GS", "HEAD"],
            )
            .expect("verification should run");
        let parsed = parse_signature(&verified.stdout);
        assert_eq!(parsed.status, CommitSignatureStatus::Good);
        assert_eq!(parsed.signer, "Gitronimo Test <test@gitronimo.invalid>");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn lists_tracked_files_from_the_index() {
        let repository = Repository::new();
        fs::create_dir_all(repository.path.join("src")).expect("src should exist");
        fs::write(repository.path.join("README.md"), "readme\n").expect("readme should write");
        fs::write(repository.path.join("src/lib.rs"), "fn main() {}\n").expect("lib should write");
        fs::write(repository.path.join("ignored.tmp"), "temp\n").expect("temp should write");
        repository.success(["add", "README.md", "src/lib.rs"]);
        let RepositoryLocation::Worktree(worktree) = repository
            .git
            .discover_repository(&repository.path)
            .expect("fixture should be a worktree")
        else {
            panic!("fixture should be a working tree");
        };

        let files = repository
            .git
            .tracked_files(&worktree)
            .expect("tracked files should list");
        assert_eq!(
            files,
            vec![
                GitPath(b"README.md".to_vec()),
                GitPath(b"src/lib.rs".to_vec())
            ]
        );
    }

    #[test]
    fn parse_git_progress_line_reads_percentage_tokens() {
        assert_eq!(
            parse_git_progress_line("Receiving objects:  45% (123/456)"),
            Some(0.45)
        );
        assert_eq!(
            parse_git_progress_line("Counting objects: 100% (10/10), done."),
            Some(1.0)
        );
        assert_eq!(parse_git_progress_line("remote: Enumerating objects"), None);
    }

    #[test]
    fn parse_numstat_line_reads_tab_separated_counts() {
        let (path, added, deleted) =
            parse_numstat_line("12\t3\tapps/desktop/src/main.rs").expect("numstat line");
        assert_eq!(added, 12);
        assert_eq!(deleted, 3);
        assert_eq!(path.0, b"apps/desktop/src/main.rs".to_vec());
    }
}
