use anyhow::Result;

mod forgejo;
mod github;
mod gitlab;

pub use forgejo::Forgejo;
pub use github::GitHub;
pub use gitlab::GitLab;

pub trait Forge {
    /// Open a PR on the forge.
    ///
    /// Specify the source branch for the PR and the name of the dependency that is being updated.
    fn open_pull_request(
        &self,
        source_branch: &str,
        name: &str,
        body: Option<String>,
    ) -> Result<String>;
}
