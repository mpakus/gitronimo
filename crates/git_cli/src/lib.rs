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
    BranchStatus, CommitIdentity, DiffFile, DiffHunk, DiffLine, DiffLineKind, FileStatus, GitPath,
    HeadStatus, HistoryCommit, HistoryPage, HistoryReference, HistoryRequest, NamedRef,
    RefDecoration, RefSnapshot, Remote, RenameKind, RepositoryLocation, StatusEntry,
    SubmoduleState, UnifiedDiff, WorktreeRepository, WorktreeStatus, parse_hunk_header,
    selected_lines_patch,
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
        let mut child = self
            .command(directory, args)
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
        Ok(status)
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
        let refs = self.run(
            &repository.worktree_root,
            [
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
        )?;
        if !refs.status.success() {
            return Err(command_error(&refs));
        }
        let remotes = self.run(
            &repository.worktree_root,
            ["config", "--null", "--get-regexp", "^remote\\..*\\.url$"],
        )?;
        if !remotes.status.success() && remotes.status.code() != Some(1) {
            return Err(command_error(&remotes));
        }
        parse_ref_snapshot(&refs.stdout, &remotes.stdout)
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

    /// Applies the latest stash while retaining its recovery entry.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn apply_latest_stash(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "apply", "stash@{0}"])
    }

    /// Applies and removes the latest stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's conflict or missing-stash failure.
    pub fn pop_latest_stash(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "pop", "stash@{0}"])
    }

    /// Removes the latest stash after caller confirmation.
    ///
    /// # Errors
    /// Returns Git's missing-stash failure.
    pub fn drop_latest_stash(&self, repository: &WorktreeRepository) -> Result<(), GitStatusError> {
        self.mutate(repository, ["stash", "drop", "stash@{0}"])
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
    ParseHistory,
    ParseHistoryFields,
    ParseHistoryTimestamp,
}

fn command_error(output: &Output) -> GitStatusError {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let message = if message.is_empty() {
        "Git command failed without an error message.".into()
    } else {
        message
    };
    GitStatusError::CommandFailed(message)
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

fn parse_ref_snapshot(refs: &[u8], remotes: &[u8]) -> Result<RefSnapshot, GitStatusError> {
    let fields = refs
        .split(|byte| *byte == 0)
        .filter(|field| field.iter().any(|byte| !byte.is_ascii_whitespace()))
        .collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        return Err(GitStatusError::ParseHistory);
    }
    let mut snapshot = RefSnapshot::default();
    for pair in fields.chunks_exact(2) {
        let (name, target) = (
            pair[0].trim_ascii_start(),
            std::str::from_utf8(pair[1]).map_err(|_| GitStatusError::ParseHistory)?,
        );
        let named = |prefix: &[u8]| NamedRef {
            name: GitPath(name[prefix.len()..].to_vec()),
            target: target.to_owned(),
        };
        if name.starts_with(b"refs/heads/") {
            snapshot.local_branches.push(named(b"refs/heads/"));
        } else if name.starts_with(b"refs/remotes/") {
            snapshot.remote_branches.push(named(b"refs/remotes/"));
        } else if name.starts_with(b"refs/tags/") {
            snapshot.tags.push(named(b"refs/tags/"));
        }
    }
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
        git_candidates, parse_commit_records, parse_porcelain_v2_z, parse_unified_diff,
        read_limited,
    };
    use app_core::{RepositoryDiscoverer, RepositoryOpenError, open_repository};
    use git_domain::{
        DiffLineKind, GitPath, HeadStatus, HistoryReference, HistoryRequest, RepositoryLocation,
        StatusEntry, SubmoduleState,
    };

    static NEXT_REPOSITORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn process_output_reader_rejects_oversized_streams() {
        let output = vec![b'x'; MAX_PROCESS_OUTPUT_BYTES + 1];
        let error = read_limited(Cursor::new(output)).expect_err("output should be bounded");
        assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge);
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
        repository
            .git
            .create_stash(&worktree, true)
            .expect("fixture should stash");
        repository
            .git
            .drop_latest_stash(&worktree)
            .expect("latest stash should drop");
        assert_eq!(
            repository
                .git
                .worktree_status(&worktree, false)
                .expect("status should load")
                .stash_count,
            0
        );
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
}
