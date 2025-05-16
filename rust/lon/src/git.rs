use std::{
    fmt,
    io::BufRead,
    path::Path,
    process::{Command, Output},
};

use anyhow::{Context, Result, bail};
use tempfile::TempDir;

/// A git revision (aka commit).
#[derive(PartialEq, Clone)]
pub struct Revision(String);

impl Revision {
    pub fn new(s: &str) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Output of `git ls-remote`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemoteInfo {
    pub revision: String,
    pub reference: String,
}

pub struct User {
    name: String,
    email: String,
}

impl User {
    pub fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
        }
    }
}

/// Find the newest revision for a branch of a git repository.
pub fn find_newest_revision(url: &str, branch: &str) -> Result<Revision> {
    find_newest_revision_for_ref(url, &format!("refs/heads/{branch}")).with_context(|| {
        format!(
            "Failed to find newest revision for {url} ({branch}).\nAre you sure the repo exists and contains the branch {branch}?"
        )
    })
}

/// Find the newest revision for a reference of a git repository.
fn find_newest_revision_for_ref(url: &str, reference: &str) -> Result<Revision> {
    let mut references =
        ls_remote(&["--refs", url, reference]).with_context(|| format!("Failed to reach {url}"))?;

    if references.is_empty() {
        bail!("The repository {url} doesn't contain the reference {reference}")
    }

    if references.len() > 1 {
        bail!("The reference {reference} is ambiguous and points to multiple revisions")
    }

    Ok(Revision(references.remove(0).revision))
}

/// Call `git ls-remote` with the provided args.
fn ls_remote(args: &[&str]) -> Result<Vec<RemoteInfo>> {
    let output = Command::new("git")
        .arg("ls-remote")
        .args(args)
        .output()
        .context("Failed to execute git ls-remote. Most likely it's not on PATH")?;
    if !output.status.success() {
        let status_code = output
            .status
            .code()
            .map_or_else(|| "None".into(), |code| code.to_string());
        let stderr_output = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| !line.is_empty())
            .collect::<Vec<&str>>()
            .join(" ");
        anyhow::bail!("git ls-remote failed with exit code {status_code}:\n{stderr_output}",);
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (revision, reference) = line.split_once('\t').ok_or_else(|| {
                anyhow::format_err!("git ls-remote output line contains no '\\t'")
            })?;
            if reference.contains('\t') {
                bail!("git ls-remote output line contains more than one '\\t'")
            }
            Ok(RemoteInfo {
                revision: revision.into(),
                reference: reference.into(),
            })
        })
        .collect::<Result<Vec<RemoteInfo>>>()
}

/// Obtain the lastModified information
pub fn get_last_modified(url: &str, rev: &str) -> Result<u64> {
    let tmp_dir = TempDir::new()?;
    let mut output: Output;

    // Init a new git directory
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .arg("init")
        .output()
        .context("Failed to execute git init. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to initialize a fresh git repository\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Add the repository as a remote
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args(["remote", "add", "origin", url])
        .output()
        .context("Failed to execute git remote add.")?;

    if !output.status.success() {
        bail!(
            "Failed to add the remote {}\n{}",
            url,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Fetch the locked revision
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args([
            "fetch",
            "--depth=1",
            "--no-show-forced-updates",
            "origin",
            rev,
        ])
        .output()
        .context("Failed to execute git fetch.")?;

    if !output.status.success() {
        bail!(
            "Failed to fetch the revision {}\n{}",
            rev,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Get the lastModified value
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args(["log", "-1", "--format=%ct", "--no-show-signature", rev])
        .output()
        .context("Failed to execute git log.")?;

    if !output.status.success() {
        bail!(
            "Failed to log the revision {}\n{}",
            rev,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .parse::<u64>()
        .context("Failed to parse last modified timestamp.")
}

/// List the commits between two revisions
pub fn diff_history(
    url: &str,
    old_revision: &str,
    new_revision: &str,
    num_commits: usize,
) -> Result<String> {
    let tmp_dir = TempDir::new()?;
    let mut output: Output;

    // Init a new git directory
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .arg("init")
        .output()
        .context("Failed to execute git init. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to initialize a fresh git repository\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Add the repository as a remote
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args(["remote", "add", "origin", url])
        .output()
        .context("Failed to execute git remote add.")?;

    if !output.status.success() {
        bail!(
            "Failed to add the remote {}\n{}",
            url,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Fetch the old revision
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args([
            "fetch",
            "--depth=1",
            "--no-show-forced-updates",
            "origin",
            old_revision,
        ])
        .output()
        .context("Failed to execute git fetch.")?;

    if !output.status.success() {
        bail!(
            "Failed to fetch the revision {}\n{}",
            old_revision,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Fetch the new revision, up to the old one
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args([
            "fetch",
            "--no-show-forced-updates",
            "--negotiation-tip",
            old_revision,
            "origin",
            new_revision,
        ])
        .arg(format!("--depth={num_commits}"))
        .output()
        .context("Failed to execute git fetch.")?;

    if !output.status.success() {
        bail!(
            "Failed to fetch the revision {}\n{}",
            new_revision,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    // Get the history
    output = Command::new("git")
        .arg("--git-dir")
        .arg(tmp_dir.path())
        .args(["rev-list", "--oneline"])
        .arg(format!("{old_revision}..{new_revision}"))
        .output()
        .context("Failed to execute git rev-list.")?;

    if !output.status.success() {
        bail!(
            "Failed to list the history for {}..{}\n{}",
            old_revision,
            new_revision,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    Ok(output
        .stdout
        .lines()
        .map(|s| format!("  {}", s.expect("Failed to read git rev-list output")))
        .collect::<Vec<String>>()
        .join("\n"))
}

pub fn add(directory: impl AsRef<Path>, args: &[&Path]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory.as_ref())
        .arg("add")
        .args(args)
        .output()
        .context("Failed to execute git add. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to add files to git statging\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub fn commit(directory: impl AsRef<Path>, message: &str, user: Option<User>) -> Result<()> {
    let mut command = Command::new("git");
    command.arg("-C").arg(directory.as_ref());

    if let Some(user) = user {
        command
            .arg("-c")
            .arg(format!("user.name={}", user.name))
            .arg("-c")
            .arg(format!("user.email={}", user.email));
    }

    let output = command
        .arg("commit")
        .arg("--message")
        .arg(message)
        .output()
        .context("Failed to execute git commit. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to commit files\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Retrieve the current ref.
///
/// This is either a branch or a commit (if you're on a detached HEAD).
pub fn current_rev(directory: impl AsRef<Path>) -> Result<String> {
    let symbolic_ref_output = Command::new("git")
        .arg("-C")
        .arg(directory.as_ref())
        .arg("symbolic-ref")
        .arg("--short")
        .arg("HEAD")
        .output()
        .context("Failed to execute git symbolic-ref. Most likely it's not on PATH")?;

    if symbolic_ref_output.status.success() {
        return Ok(String::from_utf8_lossy(&symbolic_ref_output.stdout)
            .trim_end()
            .into());
    }

    // If we're not on a branch, we retrieve the commit hash of the presumably detached HEAD.
    let rev_parse_output = Command::new("git")
        .arg("-C")
        .arg(directory.as_ref())
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .context("Failed to execute git rev-parse. Most likely it's not on PATH")?;

    if !rev_parse_output.status.success() {
        bail!(
            "Failed to find current commit \n{}",
            String::from_utf8_lossy(&rev_parse_output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&rev_parse_output.stdout)
        .trim_end()
        .into())
}

/// Checkout a reference.
pub fn checkout(directory: impl AsRef<Path>, reference: &str, create_or_reset: bool) -> Result<()> {
    let mut command = Command::new("git");

    command.arg("-C").arg(directory.as_ref()).arg("checkout");

    if create_or_reset {
        command.arg("-B");
    }

    command.arg(reference);

    let output = command
        .output()
        .context("Failed to execute git checkout. Most likely it's not on PATH")?;
    if !output.status.success() {
        bail!(
            "Failed to checkout ref {reference} \n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Force push the current branch to the default remote.
pub fn force_push(directory: impl AsRef<Path>, url: Option<&str>, branch: &str) -> Result<()> {
    let repository = url.unwrap_or("origin");

    let output = Command::new("git")
        .arg("-C")
        .arg(directory.as_ref())
        .arg("push")
        .arg("--force")
        .arg(repository)
        .arg(branch)
        .output()
        .context("Failed to execute git push. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to force push current branch \n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
