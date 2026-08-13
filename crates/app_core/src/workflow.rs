//! Per-repository Git branching conventions (Tower Workflows, approach-only).

use git_domain::MergeMethod;
use serde::{Deserialize, Serialize};

/// One Git mutation used while finishing or syncing a topic branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowGitStep {
    Checkout(String),
    Merge(String),
    MergeSquash(String),
    Rebase(String),
    MergeFfOnly(String),
    DeleteBranch { name: String, force: bool },
}

/// Named branching convention applied to one repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowKind {
    GitHubFlow,
    GitLabFlow,
    GitFlow,
    Custom,
}

impl WorkflowKind {
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::GitHubFlow => "GitHub Flow",
            Self::GitLabFlow => "GitLab Flow",
            Self::GitFlow => "git-flow",
            Self::Custom => "Custom",
        }
    }

    #[must_use]
    pub fn caption(self) -> &'static str {
        match self {
            Self::GitHubFlow => "One long-lived trunk. Topic branches merge back into it.",
            Self::GitLabFlow => "Trunk plus an environment base such as production or staging.",
            Self::GitFlow => "main and develop, with feature, release, and hotfix topics.",
            Self::Custom => "Base and topic types you defined for this repository.",
        }
    }
}

/// Long-lived branch in the convention (trunk or an environment line).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowBaseBranch {
    pub name: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub is_trunk: bool,
}

/// Short-lived branch type with prefix, start point, and finish targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTopicType {
    pub label: String,
    pub prefix: String,
    pub start: String,
    pub merge_into: Vec<String>,
    pub strategy: MergeMethod,
}

/// Persisted workflow for one repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryWorkflow {
    pub kind: WorkflowKind,
    pub bases: Vec<WorkflowBaseBranch>,
    pub topics: Vec<WorkflowTopicType>,
}

impl RepositoryWorkflow {
    #[must_use]
    pub fn github_flow(trunk: &str) -> Self {
        Self {
            kind: WorkflowKind::GitHubFlow,
            bases: vec![WorkflowBaseBranch {
                name: trunk.to_owned(),
                parent: None,
                is_trunk: true,
            }],
            topics: vec![topic(
                "Feature",
                "feature/",
                trunk,
                vec![trunk.to_owned()],
                MergeMethod::Merge,
            )],
        }
    }

    #[must_use]
    pub fn gitlab_flow(trunk: &str, production: &str) -> Self {
        Self {
            kind: WorkflowKind::GitLabFlow,
            bases: vec![
                WorkflowBaseBranch {
                    name: trunk.to_owned(),
                    parent: None,
                    is_trunk: true,
                },
                WorkflowBaseBranch {
                    name: production.to_owned(),
                    parent: Some(trunk.to_owned()),
                    is_trunk: false,
                },
            ],
            topics: vec![
                topic(
                    "Feature",
                    "feature/",
                    trunk,
                    vec![trunk.to_owned()],
                    MergeMethod::Merge,
                ),
                topic(
                    "Hotfix",
                    "hotfix/",
                    production,
                    vec![production.to_owned(), trunk.to_owned()],
                    MergeMethod::Merge,
                ),
            ],
        }
    }

    #[must_use]
    pub fn git_flow(trunk: &str, develop: &str) -> Self {
        Self {
            kind: WorkflowKind::GitFlow,
            bases: vec![
                WorkflowBaseBranch {
                    name: trunk.to_owned(),
                    parent: None,
                    is_trunk: true,
                },
                WorkflowBaseBranch {
                    name: develop.to_owned(),
                    parent: Some(trunk.to_owned()),
                    is_trunk: false,
                },
            ],
            topics: vec![
                topic(
                    "Feature",
                    "feature/",
                    develop,
                    vec![develop.to_owned()],
                    MergeMethod::Merge,
                ),
                topic(
                    "Release",
                    "release/",
                    develop,
                    vec![trunk.to_owned(), develop.to_owned()],
                    MergeMethod::Merge,
                ),
                topic(
                    "Hotfix",
                    "hotfix/",
                    trunk,
                    vec![trunk.to_owned(), develop.to_owned()],
                    MergeMethod::Merge,
                ),
            ],
        }
    }

    /// Infer a template from existing local branch names.
    #[must_use]
    pub fn detect(branch_names: &[String]) -> Self {
        let trunk = trunk_name(branch_names);
        let has = |needle: &str| branch_names.iter().any(|name| name == needle);
        if has("develop") {
            Self::git_flow(&trunk, "develop")
        } else if has("production") {
            Self::gitlab_flow(&trunk, "production")
        } else if has("staging") {
            Self::gitlab_flow(&trunk, "staging")
        } else {
            Self::github_flow(&trunk)
        }
    }

    /// Apply a named template, using existing branch names for trunk/env hints.
    #[must_use]
    pub fn from_kind(kind: WorkflowKind, branch_names: &[String]) -> Self {
        let trunk = trunk_name(branch_names);
        match kind {
            WorkflowKind::GitHubFlow | WorkflowKind::Custom => Self::github_flow(&trunk),
            WorkflowKind::GitLabFlow => {
                let env = if branch_names.iter().any(|name| name == "staging") {
                    "staging"
                } else {
                    "production"
                };
                Self::gitlab_flow(&trunk, env)
            }
            WorkflowKind::GitFlow => Self::git_flow(&trunk, "develop"),
        }
    }

    #[must_use]
    pub fn topic_for_branch(&self, branch: &str) -> Option<&WorkflowTopicType> {
        self.topics
            .iter()
            .filter(|topic| !topic.prefix.is_empty() && branch.starts_with(&topic.prefix))
            .max_by_key(|topic| topic.prefix.len())
    }

    /// Parent branch to merge into HEAD when syncing a topic or child base.
    #[must_use]
    pub fn sync_parent(&self, head: &str) -> Option<&str> {
        if let Some(topic) = self.topic_for_branch(head) {
            return Some(topic.start.as_str());
        }
        self.bases
            .iter()
            .find(|base| base.name == head)
            .and_then(|base| base.parent.as_deref())
    }

    /// `{prefix}{suffix}` without doubling the prefix when the user types it.
    #[must_use]
    pub fn branch_name_for_topic(prefix: &str, suffix: &str) -> String {
        let suffix = suffix.trim().trim_start_matches('/');
        let suffix = suffix
            .strip_prefix(prefix)
            .unwrap_or(suffix)
            .trim_start_matches('/');
        format!("{prefix}{suffix}")
    }

    /// Git steps that finish `topic_branch` using the topic type's merge strategy.
    #[must_use]
    pub fn finish_steps(
        topic_branch: &str,
        topic: &WorkflowTopicType,
        delete_topic: bool,
    ) -> Vec<WorkflowGitStep> {
        let mut steps = Vec::new();
        match topic.strategy {
            MergeMethod::Merge => {
                for target in &topic.merge_into {
                    steps.push(WorkflowGitStep::Checkout(target.clone()));
                    steps.push(WorkflowGitStep::Merge(topic_branch.to_owned()));
                }
            }
            MergeMethod::Squash => {
                for target in &topic.merge_into {
                    steps.push(WorkflowGitStep::Checkout(target.clone()));
                    steps.push(WorkflowGitStep::MergeSquash(topic_branch.to_owned()));
                }
            }
            MergeMethod::Rebase => {
                if let Some(first) = topic.merge_into.first() {
                    steps.push(WorkflowGitStep::Checkout(topic_branch.to_owned()));
                    steps.push(WorkflowGitStep::Rebase(first.clone()));
                    steps.push(WorkflowGitStep::Checkout(first.clone()));
                    steps.push(WorkflowGitStep::MergeFfOnly(topic_branch.to_owned()));
                    for target in topic.merge_into.iter().skip(1) {
                        steps.push(WorkflowGitStep::Checkout(target.clone()));
                        steps.push(WorkflowGitStep::Merge(topic_branch.to_owned()));
                    }
                }
            }
        }
        if delete_topic && !topic.merge_into.is_empty() {
            steps.push(WorkflowGitStep::DeleteBranch {
                name: topic_branch.to_owned(),
                force: topic.strategy == MergeMethod::Squash,
            });
        }
        steps
    }
}

fn topic(
    label: &str,
    prefix: &str,
    start: &str,
    merge_into: Vec<String>,
    strategy: MergeMethod,
) -> WorkflowTopicType {
    WorkflowTopicType {
        label: label.to_owned(),
        prefix: prefix.to_owned(),
        start: start.to_owned(),
        merge_into,
        strategy,
    }
}

fn trunk_name(branch_names: &[String]) -> String {
    if branch_names.iter().any(|name| name == "main") {
        "main".into()
    } else if branch_names.iter().any(|name| name == "master") {
        "master".into()
    } else {
        "main".into()
    }
}

#[cfg(test)]
mod tests {
    use super::{RepositoryWorkflow, WorkflowGitStep, WorkflowKind};
    use git_domain::MergeMethod;

    #[test]
    fn detects_git_flow_when_develop_exists() {
        let workflow = RepositoryWorkflow::detect(&["main".into(), "develop".into()]);
        assert_eq!(workflow.kind, WorkflowKind::GitFlow);
        assert_eq!(workflow.bases.len(), 2);
        assert_eq!(workflow.topics.len(), 3);
        assert_eq!(workflow.topics[0].start, "develop");
    }

    #[test]
    fn detects_gitlab_flow_from_production() {
        let workflow = RepositoryWorkflow::detect(&["main".into(), "production".into()]);
        assert_eq!(workflow.kind, WorkflowKind::GitLabFlow);
        assert_eq!(workflow.bases[1].name, "production");
        assert_eq!(workflow.topics[1].strategy, MergeMethod::Merge);
    }

    #[test]
    fn detects_github_flow_from_main_only() {
        let workflow = RepositoryWorkflow::detect(&["main".into(), "feature/login".into()]);
        assert_eq!(workflow.kind, WorkflowKind::GitHubFlow);
        assert_eq!(workflow.bases[0].name, "main");
        assert_eq!(
            workflow
                .topic_for_branch("feature/login")
                .map(|t| t.label.as_str()),
            Some("Feature")
        );
        assert_eq!(workflow.topic_for_branch("main"), None);
    }

    #[test]
    fn prefers_master_as_trunk_when_main_is_absent() {
        let workflow = RepositoryWorkflow::detect(&["master".into()]);
        assert_eq!(workflow.bases[0].name, "master");
    }

    #[test]
    fn from_kind_uses_staging_when_present() {
        let workflow = RepositoryWorkflow::from_kind(
            WorkflowKind::GitLabFlow,
            &["main".into(), "staging".into()],
        );
        assert_eq!(workflow.kind, WorkflowKind::GitLabFlow);
        assert_eq!(workflow.bases[1].name, "staging");
    }

    #[test]
    fn topic_branch_name_does_not_double_prefix() {
        assert_eq!(
            RepositoryWorkflow::branch_name_for_topic("feature/", "login"),
            "feature/login"
        );
        assert_eq!(
            RepositoryWorkflow::branch_name_for_topic("feature/", "feature/login"),
            "feature/login"
        );
    }

    #[test]
    fn finish_steps_merge_then_delete() {
        let workflow = RepositoryWorkflow::github_flow("main");
        let topic = &workflow.topics[0];
        assert_eq!(
            RepositoryWorkflow::finish_steps("feature/login", topic, true),
            vec![
                WorkflowGitStep::Checkout("main".into()),
                WorkflowGitStep::Merge("feature/login".into()),
                WorkflowGitStep::DeleteBranch {
                    name: "feature/login".into(),
                    force: false,
                },
            ]
        );
    }

    #[test]
    fn finish_steps_squash_force_deletes() {
        let mut workflow = RepositoryWorkflow::github_flow("main");
        workflow.topics[0].strategy = MergeMethod::Squash;
        let topic = &workflow.topics[0];
        let steps = RepositoryWorkflow::finish_steps("feature/login", topic, true);
        assert!(matches!(
            steps.last(),
            Some(WorkflowGitStep::DeleteBranch { force: true, .. })
        ));
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, WorkflowGitStep::MergeSquash(_)))
        );
    }

    #[test]
    fn finish_steps_rebase_then_merges_remaining_targets() {
        let workflow = RepositoryWorkflow::git_flow("main", "develop");
        let mut topic = workflow.topics[1].clone();
        topic.strategy = MergeMethod::Rebase;
        assert_eq!(
            RepositoryWorkflow::finish_steps("release/1.0", &topic, false),
            vec![
                WorkflowGitStep::Checkout("release/1.0".into()),
                WorkflowGitStep::Rebase("main".into()),
                WorkflowGitStep::Checkout("main".into()),
                WorkflowGitStep::MergeFfOnly("release/1.0".into()),
                WorkflowGitStep::Checkout("develop".into()),
                WorkflowGitStep::Merge("release/1.0".into()),
            ]
        );
    }

    #[test]
    fn sync_parent_uses_topic_start_or_base_parent() {
        let workflow = RepositoryWorkflow::git_flow("main", "develop");
        assert_eq!(workflow.sync_parent("feature/login"), Some("develop"));
        assert_eq!(workflow.sync_parent("develop"), Some("main"));
        assert_eq!(workflow.sync_parent("main"), None);
    }
}
