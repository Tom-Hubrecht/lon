use std::env;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{bot::Forge, config::required_env};

pub struct GitLab {
    // Defined by CI
    api_url: String,
    project_id: String,
    default_branch: String,

    // Defined by the user
    labels: Vec<String>,
    token: String,
}

impl GitLab {
    pub fn from_env() -> Result<Self> {
        let labels = env::var("LON_LABELS").unwrap_or_default();
        Ok(Self {
            api_url: required_env("CI_API_V4_URL")?,
            project_id: required_env("CI_PROJECT_ID")?,
            default_branch: required_env("CI_DEFAULT_BRANCH")?,

            labels: labels.split(',').map(ToString::to_string).collect(),
            token: required_env("LON_TOKEN")?,
        })
    }

    fn project_api_url(&self) -> String {
        format!("{}/projects/{}", self.api_url, self.project_id)
    }
}

impl Forge for GitLab {
    fn open_pull_request(&self, branch: &str, name: &str, body: Option<String>) -> Result<String> {
        let merge_request = MergeRequest {
            source_branch: branch.into(),
            target_branch: self.default_branch.clone(),
            title: format!("lon: update {name}"),
            body,
            remove_source_branch: true,
            allow_collaboration: true,
            labels: self.labels.join(","),
        };

        let url = format!("{}/merge_requests", self.project_api_url());

        let client = reqwest::blocking::Client::new();
        let res = client
            .post(&url)
            .json(&merge_request)
            .bearer_auth(&self.token)
            .send()
            .with_context(|| format!("Failed to send POST request to {url}"))?;

        let status = res.status();
        if !status.is_success() {
            bail!("Failed to open Merge Request at {url}: {status}")
        }

        let res_json = res.json::<MergeRequestResponse>()?;

        Ok(res_json.web_url)
    }
}

#[derive(Serialize)]
struct MergeRequest {
    source_branch: String,
    target_branch: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    remove_source_branch: bool,
    allow_collaboration: bool,
    labels: String,
}

#[derive(Deserialize)]
struct MergeRequestResponse {
    web_url: String,
}
