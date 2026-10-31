//! Bounded GitHub/GitLab API adapters. No Tauri or Git checkout mutations.
mod credential_store;
pub mod git_credentials;
pub mod jenkins;
mod repository;
mod secure_prompt;
use gitnova_domain::*;
use reqwest::{blocking::Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, time::Duration};
use url::Url;
fn err(message: &str) -> AppError {
    AppError::new("hosting", message)
}
pub fn validated_ci_url(raw: &str) -> Result<String> {
    if raw.len() > 4096 || raw.chars().any(char::is_control) {
        return Err(err("Choose a credential-free HTTPS build link."));
    }
    let url = Url::parse(raw).map_err(|_| err("Choose a credential-free HTTPS build link."))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(err("Choose a credential-free HTTPS build link."));
    }
    Ok(url.into())
}
pub fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
fn decode(value: &str) -> Result<String> {
    let mut bytes = vec![];
    let raw = value.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() {
                return Err(err("Invalid encoded project path."));
            }
            bytes.push(
                u8::from_str_radix(&value[i + 1..i + 3], 16)
                    .map_err(|_| err("Invalid encoded project path."))?,
            );
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| err("Project paths must be UTF-8."))
}
/// Normalize an account identity without inventing a repository or revision.
pub fn host_context(profile: &HostProfile) -> Result<HostingContext> {
    if !matches!(profile.provider.as_str(), "gitlab" | "github") {
        return Err(err("Choose GitHub or GitLab."));
    }
    let origin = validated_ci_url(&profile.origin)?;
    let url = Url::parse(&origin).map_err(|_| err("Invalid host."))?;
    if url.query().is_some()
        || url.fragment().is_some()
        || url.path().split('/').any(|p| p == ".." || p == ".")
    {
        return Err(err(
            "Provider addresses require HTTPS without credentials, query or fragment.",
        ));
    }
    let origin = origin.trim_end_matches('/').to_owned();
    Ok(HostingContext {
        selection: HostingSelection {
            remote: String::new(),
            branch: String::new(),
            remote_branch: None,
            oid: String::new(),
            provider: Some(profile.provider.clone()),
            origin: Some(origin.clone()),
        },
        provider: profile.provider.clone(),
        origin,
        project: String::new(),
        account: None,
    })
}
/// Match only the configured fetch host, never a substring or a repository name.
pub fn matches_host(profile: &HostProfile, remote: &str) -> Result<bool> {
    let context = host_context(profile)?;
    let origin = Url::parse(&context.origin).map_err(|_| err("Invalid host."))?;
    let parsed = if remote.contains("://") {
        Url::parse(remote).ok()
    } else {
        remote.split_once(':').and_then(|(host, path)| {
            Url::parse(&format!("ssh://{host}/{}", path.trim_start_matches('/'))).ok()
        })
    };
    let Some(url) = parsed else { return Ok(false) };
    if !matches!(url.scheme(), "https" | "ssh")
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.scheme() == "https" && !url.username().is_empty())
        || url.host_str() != origin.host_str()
    {
        return Ok(false);
    }
    if url.scheme() == "https" {
        if url.port_or_known_default() != origin.port_or_known_default() {
            return Ok(false);
        }
        let prefix = origin.path().trim_end_matches('/');
        if !prefix.is_empty() && !url.path().starts_with(&format!("{prefix}/")) {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn resolve(selection: &HostingSelection, remote_url: &str) -> Result<HostingContext> {
    if remote_url.len() > 4096 {
        return Err(err("Remote URL is too long."));
    }
    let (host, mut project) = if remote_url.contains("://") {
        let url =
            Url::parse(remote_url).map_err(|_| err("Choose a GitHub or GitLab remote URL."))?;
        if !matches!(url.scheme(), "https" | "ssh")
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(err("Choose a credential-free HTTPS or SSH remote."));
        }
        (
            url.host_str()
                .ok_or_else(|| err("Remote host is missing."))?
                .to_owned(),
            decode(url.path().trim_start_matches('/'))?,
        )
    } else {
        let (host, path) = remote_url
            .split_once(':')
            .ok_or_else(|| err("Choose a GitHub or GitLab remote rather than a local path."))?;
        (
            host.rsplit_once('@')
                .map_or(host, |(_, host)| host)
                .to_owned(),
            path.trim_start_matches('/').to_owned(),
        )
    };
    let host = host.to_ascii_lowercase();
    if selection.origin.as_ref().is_some_and(|s| s.len() > 4096) {
        return Err(err("Provider addresses must be at most 4 KiB."));
    }
    let provider = selection
        .provider
        .clone()
        .or_else(|| match host.as_str() {
            "github.com" => Some("github".into()),
            "gitlab.com" => Some("gitlab".into()),
            _ => None,
        })
        .ok_or_else(|| err("Choose GitHub or GitLab for this host."))?;
    if !matches!(provider.as_str(), "github" | "gitlab") {
        return Err(err("Choose GitHub or GitLab."));
    }
    let origin = selection
        .origin
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("https://{host}"));
    let origin_url =
        Url::parse(&origin).map_err(|_| err("Enter the provider's HTTPS web address."))?;
    if origin_url.scheme() != "https"
        || origin_url.host_str().is_none()
        || !origin_url.username().is_empty()
        || origin_url.password().is_some()
        || origin_url.query().is_some()
        || origin_url.fragment().is_some()
    {
        return Err(err(
            "Provider addresses require HTTPS without credentials, query or fragment.",
        ));
    }
    let origin = origin_url.as_str().trim_end_matches('/').to_owned();
    if remote_url.starts_with("https://") && origin_url.host_str() == Some(host.as_str()) {
        let prefix = origin_url.path().trim_matches('/');
        if !prefix.is_empty() {
            if let Some(rest) = project.strip_prefix(&format!("{prefix}/")) {
                project = rest.to_owned();
            }
        }
    }
    project = project
        .strip_suffix(".git")
        .unwrap_or(&project)
        .trim_end_matches('/')
        .to_owned();
    validate_project(&project, &provider)?;
    Ok(HostingContext {
        selection: selection.clone(),
        provider,
        origin,
        project,
        account: None,
    })
}
/// A shareable full-revision URL, built locally without credentials or a network request.
pub fn commit_url(context: &HostingContext, oid: &str) -> Result<String> {
    if !valid_oid(oid) {
        return Err(err("Choose a full commit SHA."));
    }
    let project = context
        .project
        .split('/')
        .map(encode)
        .collect::<Vec<_>>()
        .join("/");
    let route = match context.provider.as_str() {
        "gitlab" => "-/commit",
        "github" => "commit",
        _ => return Err(err("Choose GitHub or GitLab.")),
    };
    Ok(format!("{}/{}/{}/{}", context.origin, project, route, oid))
}

fn validate_project(project: &str, provider: &str) -> Result<()> {
    let parts = project.split('/').collect::<Vec<_>>();
    if project.len() > 1024
        || parts.len() < 2
        || provider == "github" && parts.len() != 2
        || parts.iter().any(|v| {
            v.is_empty()
                || *v == "."
                || *v == ".."
                || v.chars()
                    .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'))
        })
    {
        return Err(err(
            "Enter a complete provider project path, such as team/project.",
        ));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
pub struct Credential {
    pub token: String,
    pub login: String,
}
fn credential_account(c: &HostingContext) -> String {
    format!("{}:{}", c.provider, c.origin)
}
pub fn credential(c: &HostingContext) -> Result<Credential> {
    let bytes = credential_store::read("gitnova.hosting.v1", &credential_account(c))?
        .ok_or_else(|| err("Connect a provider account for this host first."))?;
    serde_json::from_slice(&bytes).map_err(|_| err("Reconnect this provider account."))
}
pub(crate) use secure_prompt::secure_token;
pub fn connect(c: &HostingContext, prompt: &CredentialPrompt) -> Result<String> {
    let token = secure_token(prompt, &format!("{} at {}", c.provider, c.origin))?;
    let mut cred = Credential {
        token,
        login: String::new(),
    };
    cred.login = HostingClient::new(c, &cred)?.login()?;
    credential_store::write(
        "gitnova.hosting.v1",
        &credential_account(c),
        &serde_json::to_vec(&cred).map_err(|_| err("Could not encode account credentials."))?,
    )?;
    Ok(cred.login)
}
pub struct HostingClient<'a> {
    context: &'a HostingContext,
    credential: &'a Credential,
    client: Client,
    base: String,
}
impl<'a> HostingClient<'a> {
    pub fn new(context: &'a HostingContext, credential: &'a Credential) -> Result<Self> {
        let base = if context.provider == "github" {
            if context.origin == "https://github.com" {
                "https://api.github.com".into()
            } else {
                format!("{}/api/v3", context.origin)
            }
        } else {
            format!("{}/api/v4", context.origin)
        };
        let client = Client::builder()
            .use_native_tls()
            .user_agent("GitNova/0.1")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .pool_max_idle_per_host(1)
            .build()
            .map_err(|_| err("Could not initialize the provider connection."))?;
        Ok(Self {
            context,
            credential,
            client,
            base,
        })
    }
    fn call(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Option<Value>> {
        let posting = method == Method::POST;
        let mut req = self
            .client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.credential.token)
            .header("Accept", "application/json")
            .query(query);
        if self.context.provider == "github" {
            req = req.header("X-GitHub-Api-Version", "2022-11-28");
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        let response = req.send().map_err(|_| {
            AppError::new(
                if posting {
                    "operation_unknown"
                } else {
                    "hosting_network"
                },
                if posting {
                    "Provider write could not be confirmed. Inspect the provider before retrying."
                } else {
                    "Provider request failed. Check the connection and account, then review again."
                },
            )
        })?;
        let status = response.status();
        if status.as_u16() == 404 && !posting {
            return Ok(None);
        }
        let mut bytes = vec![];
        response.take(2*1024*1024+1).read_to_end(&mut bytes).map_err(|_|AppError::new(if posting{"operation_unknown"}else{"hosting_network"},"Provider response could not be read. Inspect existing requests before retrying."))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(AppError::new(
                if posting {
                    "operation_unknown"
                } else {
                    "hosting"
                },
                "Provider response exceeded the 2 MiB limit. Inspect the provider before retrying.",
            ));
        }
        if !status.is_success() {
            let message=match status.as_u16(){
                401=>"Provider authentication expired or was rejected. Reconnect the account and review again.".to_owned(),
                403=>"Provider denied access. Check repository permissions, draft support and branch rules.".to_owned(),
                429=>"Provider rate limit reached. Wait before reviewing again; no request was retried.".to_owned(),
                300..=399=>"Provider redirected the request. Verify the HTTPS host settings before connecting again.".to_owned(),
                422=>"The provider rejected the request details or found an existing request. Refresh and review the title and branches.".to_owned(),
                _=>format!("Provider rejected the request (HTTP {}). Refresh existing requests before retrying.",status.as_u16())
            };
            return Err(AppError::new(
                if posting && status.is_server_error() {
                    "operation_unknown"
                } else {
                    "hosting"
                },
                message,
            ));
        }
        serde_json::from_slice(&bytes).map(Some).map_err(|_| {
            AppError::new(
                if posting {
                    "operation_unknown"
                } else {
                    "hosting"
                },
                "Invalid provider response. Inspect existing requests before retrying.",
            )
        })
    }
    /// Lightweight exact-SHA graph evidence: GitLab last_pipeline when complete,
    /// otherwise its commit status. One request; no jobs, artifacts or logs.
    pub fn ci_summary(&self, oid: &str) -> Result<PipelineSnapshot> {
        if !valid_oid(oid) {
            return Err(err(
                "Choose GitLab and a full commit SHA for pipeline status.",
            ));
        }
        if self.context.provider == "github" {
            return self.github_ci_summary(oid);
        }
        let project = encode(&self.context.project);
        let row = self
            .call(
                Method::GET,
                &format!("/projects/{project}/repository/commits/{oid}"),
                &[("stats", "false".into())],
                None,
            )?
            .ok_or_else(|| {
                err("Commit is not published on the connected remote or is not accessible.")
            })?;
        if row["id"].as_str() != Some(oid) {
            return Err(err("Pipeline revision did not match the selected commit."));
        }
        let mut runs = Vec::new();
        let pipeline = &row["last_pipeline"];
        if !pipeline.is_null() && (pipeline.get("id").is_some() || pipeline.get("sha").is_some()) {
            if pipeline["sha"].as_str() != Some(oid) {
                return Err(err("Pipeline revision did not match the selected commit."));
            }
            let id = pipeline_id(pipeline)?;
            let project_path = self
                .context
                .project
                .split('/')
                .map(encode)
                .collect::<Vec<_>>()
                .join("/");
            let url = format!("{}/{project_path}/-/pipelines/{id}", self.context.origin);
            // Older GitLab versions omit web_url. Construct the canonical exact-
            // project route; if supplied, it must identify this same pipeline.
            if let Some(supplied) = pipeline.get("web_url").filter(|v| !v.is_null()) {
                let supplied = supplied
                    .as_str()
                    .ok_or_else(|| err("Invalid pipeline URL."))?;
                let parsed = Url::parse(supplied).map_err(|_| err("Invalid pipeline URL."))?;
                let expected = Url::parse(&url).map_err(|_| err("Invalid pipeline URL."))?;
                if parsed != expected
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err(err("Pipeline URL does not match the connected project."));
                }
            }
            runs.push(PipelineRun {
                display_number: None,
                name: None,
                id,
                sha: oid.into(),
                reference: bounded_field(pipeline, "ref"),
                source: "gitlab_pipeline".into(),
                status: pipeline_status(pipeline),
                url,
                jobs: Vec::new(),
                jobs_partial: false,
            });
        } else if !row["status"].is_null() {
            runs.push(PipelineRun {
                display_number: None,
                name: None,
                id: oid.into(),
                sha: oid.into(),
                reference: String::new(),
                source: "gitlab_commit_status".into(),
                status: pipeline_status(&row),
                url: commit_url(self.context, oid)?,
                jobs: Vec::new(),
                jobs_partial: false,
            });
        }
        Ok(PipelineSnapshot {
            oid: oid.into(),
            project: self.context.project.clone(),
            origin: self.context.origin.clone(),
            runs,
            partial: false,
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
    fn github_ci_summary(&self, oid: &str) -> Result<PipelineSnapshot> {
        let project = self
            .context
            .project
            .split('/')
            .map(encode)
            .collect::<Vec<_>>()
            .join("/");
        let status = self
            .call(
                Method::GET,
                &format!("/repos/{project}/commits/{oid}/status"),
                &[("per_page", "1".into())],
                None,
            )?
            .ok_or_else(|| {
                err("Commit is not published on the connected remote or is not accessible.")
            })?;
        if status["sha"].as_str() != Some(oid) {
            return Err(err("Pipeline revision did not match the selected commit."));
        }
        let checks = self
            .call(
                Method::GET,
                &format!("/repos/{project}/commits/{oid}/check-runs"),
                &[("per_page", "100".into()), ("filter", "latest".into())],
                None,
            )?
            .ok_or_else(|| err("GitHub checks are not accessible."))?;
        let rows = checks["check_runs"]
            .as_array()
            .ok_or_else(|| err("Invalid GitHub checks response."))?;
        let total = checks["total_count"]
            .as_u64()
            .ok_or_else(|| err("Invalid GitHub checks response."))?;
        if total < rows.len() as u64 {
            return Err(err("Invalid GitHub checks response."));
        }
        let mut partial = total > rows.len() as u64 || rows.len() > 100;
        let mut states = Vec::new();
        if status["total_count"]
            .as_u64()
            .ok_or_else(|| err("Invalid GitHub status response."))?
            > 0
        {
            states.push(match status["state"].as_str() {
                Some("success") => "success",
                Some("failure" | "error") => "failed",
                Some("pending") => "pending",
                _ => "unknown",
            });
        }
        for row in rows.iter().take(100) {
            if row["head_sha"].as_str() != Some(oid) {
                return Err(err("Pipeline revision did not match the selected commit."));
            }
            states.push(github_run_status(row));
        }
        // One bounded workflow metadata read, never per-job/log/artifact requests.
        // Actions permission is independent of checks permission: retain known
        // check evidence if workflow metadata is unavailable, marked incomplete.
        let actions = self
            .call(
                Method::GET,
                &format!("/repos/{project}/actions/runs"),
                &[("head_sha", oid.into()), ("per_page", "20".into())],
                None,
            )
            .ok()
            .flatten();
        let mut workflows = Vec::new();
        if let Some(actions) = actions {
            let rows = actions["workflow_runs"]
                .as_array()
                .ok_or_else(|| err("Invalid GitHub Actions response."))?;
            let total = actions["total_count"]
                .as_u64()
                .ok_or_else(|| err("Invalid GitHub Actions response."))?;
            if total < rows.len() as u64 {
                return Err(err("Invalid GitHub Actions response."));
            }
            partial |= total > rows.len() as u64 || rows.len() > 20;
            let mut latest = std::collections::BTreeMap::<String, (u64, PipelineRun)>::new();
            for row in rows.iter().take(20) {
                if row["head_sha"].as_str() != Some(oid) {
                    return Err(err("Pipeline revision did not match the selected commit."));
                }
                let id = pipeline_id(row)?;
                let number = row["run_number"]
                    .as_u64()
                    .filter(|v| *v > 0)
                    .ok_or_else(|| err("Invalid GitHub workflow number."))?;
                let workflow = value_id(&row["workflow_id"])?;
                let url = format!("{}/{project}/actions/runs/{id}", self.context.origin);
                let supplied = row["html_url"]
                    .as_str()
                    .ok_or_else(|| err("Workflow URL is missing."))?;
                if Url::parse(supplied).ok() != Url::parse(&url).ok() {
                    return Err(err("Workflow URL does not match the connected project."));
                }
                let state = github_run_status(row);
                let rank = id.parse::<u64>().map_err(|_| err("Invalid workflow ID."))?;
                let run = PipelineRun {
                    id,
                    display_number: Some(number),
                    name: Some(bounded_field(row, "name")),
                    sha: oid.into(),
                    reference: bounded_field(row, "head_branch"),
                    source: "github_actions".into(),
                    status: state.into(),
                    url,
                    jobs: Vec::new(),
                    jobs_partial: false,
                };
                if latest.get(&workflow).is_none_or(|(old, _)| rank > *old) {
                    latest.insert(workflow, (rank, run));
                }
            }
            workflows = latest.into_values().map(|(_, run)| run).collect();
            states.extend(workflows.iter().map(|run| run.status.as_str()));
        } else {
            partial = true;
        }
        if partial {
            states.push("unknown");
        }
        let aggregate = [
            "failed", "running", "pending", "unknown", "canceled", "success", "skipped",
        ]
        .into_iter()
        .find(|s| states.contains(s));
        let mut runs: Vec<PipelineRun> = aggregate
            .map(|state| PipelineRun {
                display_number: None,
                name: None,
                id: oid.into(),
                sha: oid.into(),
                reference: String::new(),
                source: "github_status_and_checks".into(),
                status: state.into(),
                url: commit_url(self.context, oid).unwrap_or_default(),
                jobs: Vec::new(),
                jobs_partial: partial,
            })
            .into_iter()
            .collect();
        runs.extend(workflows);
        Ok(PipelineSnapshot {
            oid: oid.into(),
            project: self.context.project.clone(),
            origin: self.context.origin.clone(),
            runs,
            partial,
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
    pub fn pipelines(&self, oid: &str) -> Result<PipelineSnapshot> {
        if self.context.provider != "gitlab" || !valid_oid(oid) {
            return Err(err(
                "Choose GitLab and a full commit SHA for pipeline status.",
            ));
        }
        let project = encode(&self.context.project);
        let rows = self
            .call(
                Method::GET,
                &format!("/projects/{project}/pipelines"),
                &[
                    ("sha", oid.into()),
                    ("per_page", "6".into()),
                    ("order_by", "id".into()),
                    ("sort", "desc".into()),
                ],
                None,
            )?
            .ok_or_else(|| err("GitLab project was not found or is not accessible."))?;
        let rows = rows
            .as_array()
            .ok_or_else(|| err("Invalid pipeline response."))?;
        let mut runs = Vec::new();
        for row in rows.iter().take(5) {
            if row["sha"].as_str() != Some(oid) {
                return Err(err("Pipeline revision did not match the selected commit."));
            }
            let id = pipeline_id(row)?;
            let url = row["web_url"]
                .as_str()
                .ok_or_else(|| err("Pipeline URL is missing."))?;
            let parsed = Url::parse(url).map_err(|_| err("Invalid pipeline URL."))?;
            let origin = Url::parse(&self.context.origin).map_err(|_| err("Invalid host."))?;
            if url.len() > 4096
                || parsed.scheme() != "https"
                || parsed.origin() != origin.origin()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || url.contains(&self.credential.token)
            {
                return Err(err("Pipeline URL does not match the connected host."));
            }
            let jobs = self
                .call(
                    Method::GET,
                    &format!("/projects/{project}/pipelines/{id}/jobs"),
                    &[("per_page", "51".into())],
                    None,
                )?
                .ok_or_else(|| err("Pipeline jobs are not accessible."))?;
            let jobs = jobs
                .as_array()
                .ok_or_else(|| err("Invalid pipeline jobs response."))?;
            runs.push(PipelineRun {
                display_number: None,
                name: None,
                id,
                sha: oid.into(),
                reference: bounded_field(row, "ref"),
                source: bounded_field(row, "source"),
                status: pipeline_status(row),
                url: url.into(),
                jobs_partial: jobs.len() > 50,
                jobs: jobs
                    .iter()
                    .take(50)
                    .map(|job| {
                        Ok(PipelineJob {
                            id: pipeline_id(job)?,
                            name: bounded_field(job, "name"),
                            stage: bounded_field(job, "stage"),
                            status: pipeline_status(job),
                            runner: job["runner"]["description"]
                                .as_str()
                                .map(|s| s.chars().take(256).collect()),
                            duration: job["duration"]
                                .as_f64()
                                .filter(|v| v.is_finite() && *v >= 0.0),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            });
        }
        Ok(PipelineSnapshot {
            oid: oid.into(),
            project: self.context.project.clone(),
            origin: self.context.origin.clone(),
            runs,
            partial: rows.len() > 5,
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
    pub fn login(&self) -> Result<String> {
        let v = self
            .call(Method::GET, "/user", &[], None)?
            .ok_or_else(|| err("Provider account could not be verified."))?;
        value_string(
            &v,
            if self.context.provider == "github" {
                "login"
            } else {
                "username"
            },
        )
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 256
                && !s.contains(&self.credential.token)
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        })
        .ok_or_else(|| err("Provider account identity is unavailable."))
    }
    fn project(&self, path: &str) -> Result<Value> {
        validate_project(path, &self.context.provider)?;
        let endpoint = if self.context.provider == "github" {
            format!(
                "/repos/{}",
                path.split('/').map(encode).collect::<Vec<_>>().join("/")
            )
        } else {
            format!("/projects/{}", encode(path))
        };
        self.call(Method::GET, &endpoint, &[], None)?
            .ok_or_else(|| err("Project was not found or the account cannot access it."))
    }
    fn project_endpoint(&self, project: &Value) -> Result<String> {
        if self.context.provider == "github" {
            let full = value_string(project, "full_name")
                .ok_or_else(|| err("Project identity is unavailable."))?;
            validate_project(&full, "github")?;
            Ok(format!(
                "/repos/{}",
                full.split('/').map(encode).collect::<Vec<_>>().join("/")
            ))
        } else {
            Ok(format!("/projects/{}", project_id(project)?))
        }
    }
    fn branch_oid(&self, project: &Value, branch: &str) -> Result<Option<String>> {
        let endpoint = self.project_endpoint(project)?;
        let path = if self.context.provider == "github" {
            format!("{endpoint}/branches/{}", encode(branch))
        } else {
            format!("{endpoint}/repository/branches/{}", encode(branch))
        };
        let oid = self.call(Method::GET, &path, &[], None)?.and_then(|v| {
            v["commit"][if self.context.provider == "github" {
                "sha"
            } else {
                "id"
            }]
            .as_str()
            .map(str::to_owned)
        });
        if oid
            .as_ref()
            .is_some_and(|s| !valid_oid(s) || s == &self.credential.token)
        {
            return Err(err("Provider returned an invalid commit identity."));
        }
        Ok(oid)
    }
    fn parse_request(&self, v: &Value) -> Result<HostedRequest> {
        let github = self.context.provider == "github";
        let url = value_string(v, if github { "html_url" } else { "web_url" })
            .ok_or_else(|| err("The provider did not return a request URL."))?;
        let parsed =
            Url::parse(&url).map_err(|_| err("The provider returned an invalid request URL."))?;
        let origin = Url::parse(&self.context.origin).map_err(|_| err("Invalid host."))?;
        if parsed.scheme() != "https"
            || parsed.origin() != origin.origin()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || url.contains(&self.credential.token)
        {
            return Err(err(
                "The provider returned a request URL for a different host.",
            ));
        }
        Ok(HostedRequest {
            id: project_id(v)?,
            number: v[if github { "number" } else { "iid" }]
                .as_u64()
                .ok_or_else(|| err("Request number is unavailable."))?,
            title: value_string(v, "title")
                .unwrap_or_default()
                .replace(&self.credential.token, "[redacted]"),
            body: value_string(v, if github { "body" } else { "description" })
                .unwrap_or_default()
                .replace(&self.credential.token, "[redacted]"),
            state: if !v["merged_at"].is_null() {
                "merged".into()
            } else {
                value_string(v, "state")
                    .filter(|s| matches!(s.as_str(), "open" | "opened" | "closed" | "merged"))
                    .unwrap_or_else(|| "unknown".into())
            },
            draft: v["draft"].as_bool().unwrap_or_else(|| {
                value_string(v, "title")
                    .is_some_and(|s| s.starts_with("Draft:") || s.starts_with("WIP:"))
            }),
            url,
            source_branch: if github {
                value_string(&v["head"], "ref").unwrap_or_default()
            } else {
                value_string(v, "source_branch").unwrap_or_default()
            },
            target_branch: if github {
                value_string(&v["base"], "ref").unwrap_or_default()
            } else {
                value_string(v, "target_branch").unwrap_or_default()
            },
            source_oid: if github {
                value_string(&v["head"], "sha")
                    .filter(|s| valid_oid(s) && s != &self.credential.token)
            } else {
                value_string(v, "sha").or_else(|| value_string(&v["diff_refs"], "head_sha"))
            },
        })
    }
    fn existing(
        &self,
        source: &Value,
        target: &Value,
        branch: &str,
        base: &str,
    ) -> Result<Option<HostedRequest>> {
        let github = self.context.provider == "github";
        let endpoint = self.project_endpoint(target)?;
        let source_id = project_id(source)?;
        let source_owner = source["owner"]["login"].as_str().unwrap_or("");
        for page in 1..=5 {
            let mut query = vec![
                (
                    "state",
                    if github {
                        "open".into()
                    } else {
                        "opened".into()
                    },
                ),
                ("per_page", "100".into()),
                ("page", page.to_string()),
            ];
            if github {
                query.extend([
                    ("head", format!("{source_owner}:{branch}")),
                    ("base", base.into()),
                ]);
            } else {
                query.extend([
                    ("source_branch", branch.into()),
                    ("target_branch", base.into()),
                    ("scope", "all".into()),
                ]);
            }
            let path = format!(
                "{endpoint}/{}",
                if github { "pulls" } else { "merge_requests" }
            );
            let response = self
                .call(Method::GET, &path, &query, None)?
                .ok_or_else(|| err("Request lookup is unavailable."))?;
            let rows = response
                .as_array()
                .ok_or_else(|| err("Invalid request list."))?;
            for row in rows {
                let same = if github {
                    project_id(&row["head"]["repo"]).ok().as_deref() == Some(source_id.as_str())
                        && row["head"]["ref"].as_str() == Some(branch)
                        && row["base"]["ref"].as_str() == Some(base)
                } else {
                    value_id(&row["source_project_id"]).ok().as_deref() == Some(source_id.as_str())
                        && row["source_branch"].as_str() == Some(branch)
                        && row["target_branch"].as_str() == Some(base)
                };
                if same {
                    return self.parse_request(row).map(Some);
                }
            }
            if rows.len() < 100 {
                return Ok(None);
            }
        }
        Err(err("Existing request lookup exceeded five pages. Inspect the provider before creating another request."))
    }
    fn source_branch<'b>(&self, selection: &'b HostingSelection) -> &'b str {
        selection
            .remote_branch
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&selection.branch)
    }
    pub fn review(&self, request: &HostingRequest, config_token: &str) -> Result<HostingPreview> {
        if request.target_branch.len() > 256 || request.target_branch.chars().any(char::is_control)
        {
            return Err(err("Enter a target branch up to 256 bytes."));
        }
        if request.title.trim().is_empty()
            || request.title.len() > 1024
            || request.body.len() > 65536
        {
            return Err(err(
                "Enter a title up to 1 KiB and a description up to 64 KiB.",
            ));
        }
        let source = self.project(&self.context.project)?;
        let target_path = if request.target_project.trim().is_empty() {
            &self.context.project
        } else {
            &request.target_project
        };
        let target = if target_path == &self.context.project {
            source.clone()
        } else {
            self.project(target_path)?
        };
        let mut request = request.clone();
        request.target_project = target_path.to_owned();
        if request.target_branch.is_empty() {
            request.target_branch = value_string(&target, "default_branch").ok_or_else(|| {
                err("The target project has no default branch. Enter a target branch.")
            })?;
        }
        let source_oid = self.branch_oid(&source, self.source_branch(&request.selection))?;
        let target_oid = self.branch_oid(&target, &request.target_branch)?;
        let needs_push = source_oid.as_deref() != Some(request.selection.oid.as_str());
        let existing = self.existing(
            &source,
            &target,
            self.source_branch(&request.selection),
            &request.target_branch,
        )?;
        let blocked_reason = if project_id(&source)? == project_id(&target)?
            && request.target_branch == self.source_branch(&request.selection)
        {
            Some("Choose a target branch different from the source branch.".into())
        } else if target_oid.is_none() {
            Some("The target branch does not exist on the provider.".into())
        } else if needs_push {
            Some("Publish the selected source branch revision before creating a request.".into())
        } else {
            None
        };
        let mut p = HostingPreview {
            request,
            context: self.context.clone(),
            source_project_id: project_id(&source)?,
            target_project_id: project_id(&target)?,
            source_oid,
            target_oid,
            existing,
            needs_push,
            blocked_reason,
            token: String::new(),
        };
        let bytes = serde_json::to_vec(&(&p, &self.credential.login, config_token))
            .map_err(|_| err("Could not encode request review."))?;
        p.token = format!("{:x}", Sha256::digest(bytes));
        Ok(p)
    }
    pub fn create(&self, p: &HostingPreview) -> Result<HostedRequest> {
        if let Some(existing) = &p.existing {
            return Ok(existing.clone());
        }
        if let Some(reason) = &p.blocked_reason {
            return Err(err(reason));
        }
        let github = self.context.provider == "github";
        let path = if github {
            format!(
                "/repos/{}/pulls",
                p.request
                    .target_project
                    .split('/')
                    .map(encode)
                    .collect::<Vec<_>>()
                    .join("/")
            )
        } else {
            format!("/projects/{}/merge_requests", p.source_project_id)
        };
        let body = if github {
            json!({"title":p.request.title,"body":p.request.body,"head":format!("{}:{}",self.context.project.split('/').next().unwrap_or(""),self.source_branch(&p.request.selection)),"head_repo":self.context.project.split('/').nth(1),"base":p.request.target_branch,"draft":p.request.draft})
        } else {
            json!({"title":if p.request.draft&&!p.request.title.starts_with("Draft:"){format!("Draft: {}",p.request.title)}else{p.request.title.clone()},"description":p.request.body,"source_branch":self.source_branch(&p.request.selection),"target_branch":p.request.target_branch,"target_project_id":p.target_project_id.parse::<u64>().map_err(|_|err("Invalid target project identity."))?})
        };
        let v = self
            .call(Method::POST, &path, &[], Some(body))?
            .ok_or_else(|| {
                AppError::new(
                    "operation_unknown",
                    "Provider write could not be confirmed. Inspect the provider before retrying.",
                )
            })?;
        self.parse_request(&v).map_err(|_|AppError::new("operation_unknown","The provider may have created the request, but its result could not be verified. Refresh existing requests before retrying."))
    }
}
fn valid_oid(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn value_string(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().map(str::to_owned)
}
fn project_id(v: &Value) -> Result<String> {
    value_id(&v["id"])
}
fn value_id(v: &Value) -> Result<String> {
    v.as_u64()
        .map(|id| id.to_string())
        .or_else(|| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()))
                .map(str::to_owned)
        })
        .ok_or_else(|| err("Provider identity is unavailable."))
}

pub fn disconnect(c: &HostingContext) -> Result<()> {
    credential_store::delete("gitnova.hosting.v1", &credential_account(c))
}

fn github_run_status(row: &Value) -> &'static str {
    match row["status"].as_str() {
        Some("in_progress") => "running",
        Some("queued" | "waiting" | "requested" | "pending") => "pending",
        Some("completed") => match row["conclusion"].as_str() {
            Some("success") => "success",
            Some("neutral" | "skipped") => "skipped",
            Some("failure" | "timed_out" | "action_required" | "startup_failure") => "failed",
            Some("cancelled") => "canceled",
            _ => "unknown",
        },
        _ => "unknown",
    }
}
fn pipeline_id(v: &Value) -> Result<String> {
    let id = project_id(v)?;
    if id.len() > 32 {
        return Err(err("Invalid pipeline identity."));
    }
    Ok(id)
}
fn bounded_field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").chars().take(256).collect()
}
fn pipeline_status(v: &Value) -> String {
    match v["status"].as_str().unwrap_or("unknown") {
        s @ ("created"
        | "waiting_for_resource"
        | "preparing"
        | "pending"
        | "running"
        | "success"
        | "failed"
        | "canceled"
        | "skipped"
        | "manual"
        | "scheduled"
        | "blocked") => s.into(),
        _ => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };
    type Expected = (&'static str, &'static str, u16, Value);
    fn server(expected: Vec<Expected>) -> (String, Arc<Mutex<Vec<Value>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(vec![]));
        let captured = bodies.clone();
        let task = thread::spawn(move || {
            for (method, path, status, value) in expected {
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "Missing {method} {path}");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let header_end = loop {
                    let mut chunk = [0; 2048];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend(&chunk[..n]);
                    if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                    assert!(bytes.len() < 16384);
                };
                let headers = String::from_utf8_lossy(&bytes[..header_end]).into_owned();
                assert!(
                    headers.starts_with(&format!("{method} {path}")),
                    "{headers}"
                );
                assert!(headers
                    .to_lowercase()
                    .contains("authorization: bearer mock-secret"));
                if path.ends_with("/pipelines?") {
                    assert!(
                        headers
                            .lines()
                            .next()
                            .unwrap()
                            .contains(&format!("sha={}", "a".repeat(40))),
                        "Exact SHA query missing: {headers}"
                    );
                    assert!(headers.lines().next().unwrap().contains("per_page=6"));
                }
                if path.ends_with("/actions/runs?") {
                    let request = headers.lines().next().unwrap();
                    assert!(request.contains(&format!("head_sha={}", "a".repeat(40))));
                    assert!(request.contains("per_page=20"));
                }
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let mut chunk = [0; 2048];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend(&chunk[..n]);
                }
                if length > 0 {
                    captured.lock().unwrap().push(
                        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap(),
                    );
                }
                let body = serde_json::to_string(&value).unwrap();
                write!(stream,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        (address, bodies, task)
    }
    #[test]
    fn commit_links_keep_full_revision_and_explicit_provider_paths() {
        let oid = "a".repeat(40);
        let github = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        assert_eq!(
            commit_url(&github, &oid).unwrap(),
            format!("https://github.com/team/project/commit/{oid}")
        );
        let mut config = selection("gitlab");
        config.origin = Some("https://git.example.com/gitlab".into());
        let lab = resolve(
            &config,
            "https://git.example.com/gitlab/team/sub/project.git",
        )
        .unwrap();
        assert_eq!(
            commit_url(&lab, &oid).unwrap(),
            format!("https://git.example.com/gitlab/team/sub/project/-/commit/{oid}")
        );
        assert!(commit_url(&lab, "HEAD").is_err());
        assert!(resolve(
            &config,
            "https://token:secret@git.example.com/team/project.git"
        )
        .is_err());
        assert!(resolve(&config, "/tmp/local.git").is_err());
    }
    #[test]
    fn global_host_profiles_are_exact_scoped_and_repository_independent() {
        let p = HostProfile {
            provider: "gitlab".into(),
            origin: "https://GIT.example.com/gitlab/".into(),
        };
        let context = host_context(&p).unwrap();
        assert_eq!(context.origin, "https://git.example.com/gitlab");
        assert!(context.project.is_empty());
        assert!(context.selection.oid.is_empty());
        assert!(matches_host(&p, "git@git.example.com:team/project.git").unwrap());
        assert!(matches_host(&p, "ssh://git@git.example.com:2222/team/project.git").unwrap());
        assert!(matches_host(&p, "https://git.example.com/gitlab/team/project.git").unwrap());
        for remote in [
            "https://git.example.com.evil.invalid/gitlab/team/project.git",
            "https://other.example.com/gitlab/team/project.git",
            "https://git.example.com/other/project.git",
            "https://git.example.com:8443/gitlab/team/project.git",
            "/tmp/project.git",
            "https://token@git.example.com/gitlab/team/project.git",
        ] {
            assert!(!matches_host(&p, remote).unwrap(), "{remote}");
        }
        for origin in [
            "http://git.example.com",
            "https://token:secret@git.example.com",
            "https://git.example.com?token=secret",
            "https://git.example.com/#secret",
            "file:///tmp",
        ] {
            assert!(host_context(&HostProfile {
                provider: "gitlab".into(),
                origin: origin.into()
            })
            .is_err());
        }
    }
    #[test]
    fn github_ci_summary_combines_reported_status_and_checks_without_false_green() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        for (state, conclusion, total, expected) in [
            ("success", "success", 1, "success"),
            ("failure", "success", 1, "failed"),
            ("success", "timed_out", 1, "failed"),
            ("pending", "success", 1, "pending"),
            ("success", "future", 1, "unknown"),
            ("success", "success", 2, "unknown"),
        ] {
            let (base, _, task) = server(vec![
                (
                    "GET",
                    "/repos/team/project/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/status?",
                    200,
                    json!({"sha":oid,"state":state,"total_count":1}),
                ),
                (
                    "GET",
                    "/repos/team/project/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs?",
                    200,
                    json!({"total_count":total,"check_runs":[{"head_sha":oid,"status":"completed","conclusion":conclusion}]}),
                ),
                ("GET", "/repos/team/project/actions/runs?",200,json!({"total_count":0,"workflow_runs":[]})),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let value = client.ci_summary(&oid).unwrap();
            assert_eq!(value.runs[0].status, expected);
            assert_eq!(value.partial, total > 1);
            task.join().unwrap();
        }
    }
    #[test]
    fn github_ci_summary_distinguishes_no_ci_and_rejects_wrong_revision() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let (base, _, task) = server(vec![
            (
                "GET",
                "/repos/team/project/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/status?",
                200,
                json!({"sha":oid,"state":"pending","total_count":0}),
            ),
            (
                "GET",
                "/repos/team/project/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs?",
                200,
                json!({"total_count":0,"check_runs":[]}),
            ),
            (
                "GET",
                "/repos/team/project/actions/runs?",
                200,
                json!({"total_count":0,"workflow_runs":[]}),
            ),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        assert!(client.ci_summary(&oid).unwrap().runs.is_empty());
        task.join().unwrap();
        let (base, _, task) = server(vec![(
            "GET",
            "/repos/team/project/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/status?",
            200,
            json!({"sha":"b".repeat(40),"state":"success","total_count":1}),
        )]);
        client.base = base;
        assert!(client.ci_summary(&oid).is_err());
        task.join().unwrap();
    }
    #[test]
    fn github_graph_workflows_keep_number_name_branch_and_exact_links() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let row = |id: u64, workflow: u64, number: u64, state: &str| json!({"id":id,"workflow_id":workflow,"run_number":number,"name":"CI","head_sha":oid,"head_branch":"main","status":"completed","conclusion":state,"html_url":format!("https://github.com/team/project/actions/runs/{id}")});
        for (rows, expected) in [
            (vec![row(99, 1, 1, "failure")], "failed"),
            (
                vec![row(98, 1, 1, "failure"), row(99, 1, 2, "success")],
                "success",
            ),
            (
                vec![row(99, 1, 1, "success"), row(100, 2, 1, "timed_out")],
                "failed",
            ),
        ] {
            let count = rows.len();
            let (base, _, task) = server(vec![
                (
                    "GET",
                    "/repos/team/project/commits/",
                    200,
                    json!({"sha":oid,"state":"pending","total_count":0}),
                ),
                (
                    "GET",
                    "/repos/team/project/commits/",
                    200,
                    json!({"total_count":0,"check_runs":[]}),
                ),
                (
                    "GET",
                    "/repos/team/project/actions/runs?",
                    200,
                    json!({"total_count":count,"workflow_runs":rows}),
                ),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let result = client.ci_summary(&oid).unwrap();
            assert_eq!(result.runs[0].status, expected);
            for run in result.runs.iter().skip(1) {
                assert_eq!(run.sha, oid);
                assert_eq!(run.reference, "main");
                assert_eq!(run.name.as_deref(), Some("CI"));
                assert!(run.display_number.is_some());
                assert!(run.url.ends_with(&format!("/actions/runs/{}", run.id)));
            }
            assert_eq!(
                result.runs.len(),
                if count == 2 && result.runs[1].display_number == Some(2) {
                    2
                } else {
                    count + 1
                }
            );
            task.join().unwrap();
        }
    }
    #[test]
    fn github_graph_workflows_refuse_wrong_sha_or_project_and_keep_permission_failures_incomplete()
    {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        for (sha, url, code) in [
            (
                "b".repeat(40),
                "https://github.com/team/project/actions/runs/99",
                200,
            ),
            (
                oid.clone(),
                "https://github.com/other/project/actions/runs/99",
                200,
            ),
            (
                oid.clone(),
                "https://github.com/team/project/actions/runs/99?token=secret",
                200,
            ),
            (
                oid.clone(),
                "https://github.com/team/project/actions/runs/99",
                403,
            ),
        ] {
            let (base, _, task) = server(vec![
                (
                    "GET",
                    "/repos/team/project/commits/",
                    200,
                    json!({"sha":oid,"state":"success","total_count":1}),
                ),
                (
                    "GET",
                    "/repos/team/project/commits/",
                    200,
                    json!({"total_count":0,"check_runs":[]}),
                ),
                (
                    "GET",
                    "/repos/team/project/actions/runs?",
                    code,
                    json!({"total_count":1,"workflow_runs":[{"id":99,"workflow_id":1,"run_number":1,"head_sha":sha,"head_branch":"main","status":"completed","conclusion":"success","html_url":url}]}),
                ),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let result = client.ci_summary(&oid);
            if code == 403 {
                let result = result.unwrap();
                assert!(result.partial);
                assert_eq!(result.runs[0].status, "unknown");
                assert_eq!(result.runs.len(), 1);
            } else {
                assert!(result.is_err());
            }
            task.join().unwrap();
        }
    }
    #[test]
    fn gitlab_ci_summary_uses_server_commit_status_without_job_requests() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        for status in [
            json!("running"),
            json!("failed"),
            json!(null),
            json!("future-state"),
        ] {
            let (base, _, task) = server(vec![(
                "GET",
                "/projects/team%2Fproject/repository/commits/",
                200,
                json!({"id":oid,"status":status,"last_pipeline":{"status":"success"}}),
            )]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let value = client.ci_summary(&oid).unwrap();
            if status.is_null() {
                assert!(value.runs.is_empty());
            } else {
                assert_eq!(
                    value.runs[0].status,
                    if status == "future-state" {
                        "unknown"
                    } else {
                        status.as_str().unwrap()
                    }
                );
                assert!(value.runs[0].jobs.is_empty());
                assert_eq!(
                    value.runs[0].url,
                    format!("https://gitlab.com/team/project/-/commit/{oid}")
                );
            }
            task.join().unwrap();
        }
    }
    #[test]
    fn gitlab_graph_pipeline_keeps_exact_revision_number_ref_and_state_with_one_read() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/sub/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        for status in [
            "success",
            "running",
            "pending",
            "failed",
            "canceled",
            "skipped",
            "manual",
            "future-state",
        ] {
            let (base, _, task) = server(vec![(
                "GET",
                "/projects/team%2Fsub%2Fproject/repository/commits/",
                200,
                json!({"id":oid,"status":null,"last_pipeline":{"id":40778,"sha":oid,"ref":"main","status":status,"web_url":"https://gitlab.com/team/sub/project/-/pipelines/40778"}}),
            )]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let result = client.ci_summary(&oid).unwrap();
            assert_eq!(result.runs.len(), 1);
            let run = &result.runs[0];
            assert_eq!(run.id, "40778");
            assert_eq!(run.sha, oid);
            assert_eq!(run.reference, "main");
            assert_eq!(run.source, "gitlab_pipeline");
            assert_eq!(
                run.status,
                if status == "future-state" {
                    "unknown"
                } else {
                    status
                }
            );
            assert_eq!(
                run.url,
                "https://gitlab.com/team/sub/project/-/pipelines/40778"
            );
            assert!(run.jobs.is_empty());
            assert!(!result.partial);
            task.join().unwrap();
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains("mock-secret"));
        }
    }
    #[test]
    fn gitlab_graph_pipeline_supports_base_path_and_missing_web_url() {
        let mut selected = selection("gitlab");
        selected.origin = Some("https://forge.example/gitlab".into());
        let context = resolve(
            &selected,
            "https://forge.example/gitlab/team/sub/project.git",
        )
        .unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let (base, _, task) = server(vec![(
            "GET",
            "/projects/team%2Fsub%2Fproject/repository/commits/",
            200,
            json!({"id":oid,"status":"failed","last_pipeline":{"id":"40778","sha":oid,"ref":"release","status":"success"}}),
        )]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let result = client.ci_summary(&oid).unwrap();
        assert_eq!(
            result.runs[0].url,
            "https://forge.example/gitlab/team/sub/project/-/pipelines/40778"
        );
        assert_eq!(result.runs[0].status, "success");
        task.join().unwrap();
    }
    #[test]
    fn gitlab_graph_pipeline_refuses_wrong_revision_or_destination() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let valid = json!({"id":40778,"sha":oid,"ref":"main","status":"success"});
        let mut bad_sha = valid.clone();
        bad_sha["sha"] = json!("b".repeat(40));
        let mut bad_id = valid.clone();
        bad_id["id"] = json!("not-an-id");
        let mut missing_sha = valid.clone();
        missing_sha.as_object_mut().unwrap().remove("sha");
        let mut cases = vec![bad_sha, bad_id, missing_sha];
        for url in [
            "https://evil.example/team/project/-/pipelines/40778",
            "https://gitlab.com/other/project/-/pipelines/40778",
            "https://gitlab.com/team/project/-/pipelines/40779",
            "https://alice:secret@gitlab.com/team/project/-/pipelines/40778",
            "https://gitlab.com/team/project/-/pipelines/40778?token=secret",
        ] {
            let mut row = valid.clone();
            row["web_url"] = json!(url);
            cases.push(row);
        }
        for pipeline in cases {
            let (base, _, task) = server(vec![(
                "GET",
                "/projects/team%2Fproject/repository/commits/",
                200,
                json!({"id":oid,"status":"success","last_pipeline":pipeline}),
            )]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            assert!(client.ci_summary(&oid).is_err());
            task.join().unwrap();
        }
    }
    #[test]
    fn gitlab_ci_summary_rejects_wrong_revision_and_inaccessible_commit() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        for (code, body) in [
            (200, json!({"id":"b".repeat(40),"status":"success"})),
            (404, json!({"message":"not found"})),
        ] {
            let (base, _, task) = server(vec![(
                "GET",
                "/projects/team%2Fproject/repository/commits/",
                code,
                body,
            )]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            assert!(client.ci_summary(&"a".repeat(40)).is_err());
            task.join().unwrap();
        }
    }
    #[test]
    fn gitlab_pipelines_match_exact_sha_and_keep_job_runner_states() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/sub/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let row = json!({"id":41,"sha":oid,"ref":"develop","source":"push","status":"running","web_url":"https://gitlab.com/team/sub/project/-/pipelines/41"});
        let (base, bodies, task) = server(vec![
            (
                "GET",
                "/projects/team%2Fsub%2Fproject/pipelines?",
                200,
                json!([row]),
            ),
            (
                "GET",
                "/projects/team%2Fsub%2Fproject/pipelines/41/jobs?",
                200,
                json!([
                    {"id":9,"name":"test ไทย","stage":"test","status":"running","runner":{"description":"local runner"},"duration":12.5},
                    {"id":"10","name":"deploy","stage":"deploy","status":"manual","runner":null,"duration":null}
                ]),
            ),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let result = client.pipelines(&oid).unwrap();
        assert_eq!(result.oid, oid);
        assert_eq!(
            result.runs[0].jobs[0].runner.as_deref(),
            Some("local runner")
        );
        assert_eq!(result.runs[0].jobs[1].status, "manual");
        assert!(!result.partial);
        assert_eq!(result.runs[0].source, "push");
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("mock-secret"));
        task.join().unwrap();
        assert!(bodies.lock().unwrap().is_empty());
    }
    #[test]
    fn gitlab_pipeline_refuses_wrong_sha_cross_host_and_inaccessible_project() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        for (status, payload) in [
            (200, json!([{"id":1,"sha":"b".repeat(40)}])),
            (
                200,
                json!([{"id":1,"sha":oid,"web_url":"https://evil.example/build"}]),
            ),
            (404, json!({"message":"not found"})),
            (429, json!({"message":"rate limited"})),
        ] {
            let (base, _, task) = server(vec![(
                "GET",
                "/projects/team%2Fproject/pipelines?",
                status,
                payload,
            )]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            assert!(client.pipelines(&oid).is_err());
            task.join().unwrap();
        }
    }
    #[test]
    fn gitlab_pipeline_limits_runs_jobs_and_marks_partial_coverage() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let oid = "a".repeat(40);
        let rows=(1..=6).map(|id|json!({"id":id,"sha":oid,"status":"new-future-status","web_url":format!("https://gitlab.com/team/project/-/pipelines/{id}")})).collect::<Vec<_>>();
        let jobs = (1..=51)
            .map(|id| json!({"id":id,"name":"x".repeat(512),"stage":"test","status":"success"}))
            .collect::<Vec<_>>();
        let mut expected = vec![(
            "GET",
            "/projects/team%2Fproject/pipelines?",
            200,
            json!(rows),
        )];
        // Static endpoint strings allow the shared contract server to enforce request count.
        for endpoint in [
            "/projects/team%2Fproject/pipelines/1/jobs?",
            "/projects/team%2Fproject/pipelines/2/jobs?",
            "/projects/team%2Fproject/pipelines/3/jobs?",
            "/projects/team%2Fproject/pipelines/4/jobs?",
            "/projects/team%2Fproject/pipelines/5/jobs?",
        ] {
            expected.push(("GET", endpoint, 200, json!(jobs)));
        }
        let (base, _, task) = server(expected);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let result = client.pipelines(&oid).unwrap();
        assert!(result.partial);
        assert_eq!(result.runs.len(), 5);
        assert!(result
            .runs
            .iter()
            .all(|r| r.jobs_partial && r.jobs.len() == 50 && r.status == "unknown"));
        assert_eq!(result.runs[0].jobs[0].name.len(), 256);
        task.join().unwrap();
    }
    fn selection(provider: &str) -> HostingSelection {
        HostingSelection {
            remote: "origin".into(),
            branch: "feature/test".into(),
            remote_branch: None,
            oid: "a".repeat(40),
            provider: Some(provider.into()),
            origin: None,
        }
    }
    fn request(provider: &str) -> HostingRequest {
        HostingRequest {
            selection: selection(provider),
            target_project: String::new(),
            target_branch: String::new(),
            title: "Test request".into(),
            body: "Preserve user text ไทย".into(),
            draft: true,
        }
    }
    #[test]
    fn resolves_ssh_alias_subgroup_and_web_base_path_without_guessing_a_provider() {
        let mut s = selection("gitlab");
        s.origin = Some("https://git.example/gitlab".into());
        let c = resolve(&s, "git@work-alias:group/sub/project.git").unwrap();
        assert_eq!(c.origin, "https://git.example/gitlab");
        assert_eq!(c.project, "group/sub/project");
        let c = resolve(&s, "https://git.example/gitlab/group/sub/project.git").unwrap();
        assert_eq!(c.project, "group/sub/project");
        s.provider = None;
        s.origin = None;
        assert!(resolve(&s, "git@work-alias:group/project.git").is_err());
        s.origin = Some("http://git.example".into());
        assert!(resolve(&s, "git@gitlab.com:group/project.git").is_err());
        assert!(resolve(&selection("github"), "/local/repo").is_err());
    }
    #[test]
    fn github_creates_a_draft_using_published_source_and_real_default_branch() {
        let mut req = request("github");
        req.selection.remote_branch = Some("published/test".into());
        let context = resolve(&req.selection, "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let project = json!({"id":42,"full_name":"team/project","default_branch":"main","owner":{"login":"team"}});
        let result = json!({"id":700,"number":7,"title":"Test request","body":"Preserve user text ไทย","state":"open","draft":true,"html_url":"https://github.com/team/project/pull/7","head":{"ref":"published/test","sha":"a".repeat(40),"repo":{"id":42}},"base":{"ref":"main"},"merged_at":null});
        let (base, bodies, task) = server(vec![
            ("GET", "/repos/team/project", 200, project),
            (
                "GET",
                "/repos/team/project/branches/published%2Ftest",
                200,
                json!({"commit":{"sha":"a".repeat(40)}}),
            ),
            (
                "GET",
                "/repos/team/project/branches/main",
                200,
                json!({"commit":{"sha":"b".repeat(40)}}),
            ),
            ("GET", "/repos/team/project/pulls?", 200, json!([])),
            ("POST", "/repos/team/project/pulls", 201, result),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let p = client.review(&req, "native-source-state").unwrap();
        assert!(!p.needs_push);
        assert_eq!(p.request.target_branch, "main");
        assert!(!serde_json::to_string(&p).unwrap().contains("mock-secret"));
        let r = client.create(&p).unwrap();
        assert_eq!(r.number, 7);
        assert!(r.draft);
        task.join().unwrap();
        let posts = bodies.lock().unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0]["head"], "team:published/test");
        assert_eq!(posts[0]["base"], "main");
        assert_eq!(posts[0]["body"], "Preserve user text ไทย");
    }
    #[test]
    fn gitlab_fork_request_posts_to_source_project_and_targets_the_selected_project() {
        let context = resolve(&selection("gitlab"), "git@gitlab.com:team/sub/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let mut req = request("gitlab");
        req.target_project = "team/base".into();
        let (base, bodies, task) = server(vec![
            (
                "GET",
                "/projects/team%2Fsub%2Fproject",
                200,
                json!({"id":10,"default_branch":"main"}),
            ),
            (
                "GET",
                "/projects/team%2Fbase",
                200,
                json!({"id":20,"default_branch":"develop"}),
            ),
            (
                "GET",
                "/projects/10/repository/branches/feature%2Ftest",
                200,
                json!({"commit":{"id":"a".repeat(40)}}),
            ),
            (
                "GET",
                "/projects/20/repository/branches/develop",
                200,
                json!({"commit":{"id":"b".repeat(40)}}),
            ),
            ("GET", "/projects/20/merge_requests?", 200, json!([])),
            (
                "POST",
                "/projects/10/merge_requests",
                201,
                json!({"id":800,"iid":8,"title":"Draft: Test request","description":"Preserve user text ไทย","state":"opened","web_url":"https://gitlab.com/team/base/-/merge_requests/8","source_branch":"feature/test","target_branch":"develop","sha":"a".repeat(40)}),
            ),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let p = client.review(&req, "source-state").unwrap();
        assert_eq!(p.target_project_id, "20");
        assert_eq!(client.create(&p).unwrap().number, 8);
        task.join().unwrap();
        let posts = bodies.lock().unwrap();
        assert_eq!(posts[0]["target_project_id"], 20);
        assert_eq!(posts[0]["title"], "Draft: Test request");
        assert_eq!(posts[0]["source_branch"], "feature/test");
    }
    #[test]
    fn unpublished_source_cannot_create_and_cross_host_result_urls_are_refused() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let (base, _, task) = server(vec![
            (
                "GET",
                "/repos/team/project",
                200,
                json!({"id":42,"full_name":"team/project","default_branch":"main","owner":{"login":"team"}}),
            ),
            (
                "GET",
                "/repos/team/project/branches/feature%2Ftest",
                404,
                json!({}),
            ),
            (
                "GET",
                "/repos/team/project/branches/main",
                200,
                json!({"commit":{"sha":"b".repeat(40)}}),
            ),
            ("GET", "/repos/team/project/pulls?", 200, json!([])),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let p = client.review(&request("github"), "state").unwrap();
        assert!(p.needs_push);
        assert!(client.create(&p).is_err());
        task.join().unwrap();
        assert!(client
            .parse_request(&json!({"html_url":"https://evil.example/secret"}))
            .is_err());
    }
    #[test]
    fn post_failure_is_not_retried_or_returned_as_success() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let (base, bodies, task) = server(vec![(
            "POST",
            "/repos/team/project/pulls",
            500,
            json!({"message":"do not replay"}),
        )]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let error = client
            .call(
                Method::POST,
                "/repos/team/project/pulls",
                &[],
                Some(json!({"title":"test"})),
            )
            .unwrap_err();
        assert_eq!(error.code, "operation_unknown");
        assert!(!error.message.contains("mock-secret"));
        task.join().unwrap();
        assert_eq!(bodies.lock().unwrap().len(), 1);
    }
    #[test]
    fn existing_request_is_opened_without_posting_again() {
        let context = resolve(&selection("github"), "git@github.com:team/project.git").unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let row = json!({"id":700,"number":7,"title":"Existing","body":"mock-secret is redacted","state":"open","draft":false,"html_url":"https://github.com/team/project/pull/7","head":{"ref":"feature/test","sha":"a".repeat(40),"repo":{"id":42}},"base":{"ref":"main"},"merged_at":null});
        let (base, bodies, task) = server(vec![
            (
                "GET",
                "/repos/team/project",
                200,
                json!({"id":42,"full_name":"team/project","default_branch":"main","owner":{"login":"team"}}),
            ),
            (
                "GET",
                "/repos/team/project/branches/feature%2Ftest",
                200,
                json!({"commit":{"sha":"a".repeat(40)}}),
            ),
            (
                "GET",
                "/repos/team/project/branches/main",
                200,
                json!({"commit":{"sha":"b".repeat(40)}}),
            ),
            ("GET", "/repos/team/project/pulls?", 200, json!([row])),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let p = client.review(&request("github"), "state").unwrap();
        assert!(p.existing.is_some());
        assert_eq!(client.create(&p).unwrap().number, 7);
        assert!(!serde_json::to_string(&p).unwrap().contains("mock-secret"));
        task.join().unwrap();
        assert!(bodies.lock().unwrap().is_empty());
    }
    #[test]
    fn gitlab_existing_request_matches_numeric_and_string_project_identity_without_post() {
        for identity in [json!(10), json!("10")] {
            let context = resolve(&selection("gitlab"), "git@gitlab.com:team/project.git").unwrap();
            let cred = Credential {
                token: "mock-secret".into(),
                login: "alice".into(),
            };
            let row = json!({"id":700,"iid":7,"title":"Existing MR","description":"Retain request","state":"opened","draft":false,"web_url":"https://gitlab.com/team/project/-/merge_requests/7","source_project_id":identity,"source_branch":"feature/test","target_branch":"main","sha":"a".repeat(40)});
            let (base, bodies, task) = server(vec![
                (
                    "GET",
                    "/projects/team%2Fproject",
                    200,
                    json!({"id":10,"default_branch":"main"}),
                ),
                (
                    "GET",
                    "/projects/10/repository/branches/feature%2Ftest",
                    200,
                    json!({"commit":{"id":"a".repeat(40)}}),
                ),
                (
                    "GET",
                    "/projects/10/repository/branches/main",
                    200,
                    json!({"commit":{"id":"b".repeat(40)}}),
                ),
                ("GET", "/projects/10/merge_requests?", 200, json!([row])),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let p = client.review(&request("gitlab"), "state").unwrap();
            assert!(p.existing.is_some());
            assert_eq!(client.create(&p).unwrap().number, 7);
            // Persisted plans keep the original unboxed wire shape after indirection.
            let stored = serde_json::to_value(PlannedOperation::Hosting(Box::new(p))).unwrap();
            assert_eq!(stored["type"], "hosting");
            assert_eq!(stored["preview"]["existing"]["number"], 7);
            let restored: PlannedOperation = serde_json::from_value(stored).unwrap();
            assert!(
                matches!(restored,PlannedOperation::Hosting(p) if p.existing.as_ref().is_some_and(|r|r.number==7))
            );
            task.join().unwrap();
            assert!(bodies.lock().unwrap().is_empty());
        }
    }
    #[test]
    fn hosted_repository_creation_contracts_are_empty_private_or_explicit_public_and_verified() {
        for provider in ["github", "gitlab"] {
            let github = provider == "github";
            let profile = HostProfile {
                provider: provider.into(),
                origin: if github {
                    "https://github.com"
                } else {
                    "https://git.example/gitlab"
                }
                .into(),
            };
            let context = host_context(&profile).unwrap();
            let cred = Credential {
                token: "mock-secret".into(),
                login: "alice".into(),
            };
            let request = RepositoryCreateRequest {
                profile,
                namespace_id: if github { "user:alice" } else { "42" }.into(),
                name: "demo".into(),
                description: "Project description ไทย".into(),
                visibility: if github { "private" } else { "public" }.into(),
            };
            let project = if github { "alice/demo" } else { "team/demo" };
            let clone = format!("{}/{project}.git", context.origin);
            let web = format!("{}/{project}", context.origin);
            let row = if github {
                json!({"full_name":project,"clone_url":clone,"html_url":web,"private":true})
            } else {
                json!({"path_with_namespace":project,"http_url_to_repo":clone,"web_url":web,"visibility":"public"})
            };
            let project_endpoint = if github {
                "/repos/alice/demo"
            } else {
                "/projects/team%2Fdemo"
            };
            let (base, bodies, task) = server(vec![
                (
                    "GET",
                    "/user",
                    200,
                    if github {
                        json!({"login":"alice"})
                    } else {
                        json!({"username":"alice"})
                    },
                ),
                (
                    "GET",
                    if github {
                        "/user/orgs?"
                    } else {
                        "/namespaces?"
                    },
                    200,
                    if github {
                        json!([])
                    } else {
                        json!([{"id":42,"full_path":"team"}])
                    },
                ),
                ("GET", project_endpoint, 404, json!({})),
                (
                    "POST",
                    if github { "/user/repos" } else { "/projects" },
                    201,
                    row.clone(),
                ),
                ("GET", project_endpoint, 200, row),
                (
                    "GET",
                    if github {
                        "/repos/alice/demo/branches?"
                    } else {
                        "/projects/team%2Fdemo/repository/branches?"
                    },
                    200,
                    json!([]),
                ),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            let preview = client.review_repository(&request).unwrap();
            assert!(bodies.lock().unwrap().is_empty());
            assert_eq!(preview.clone_url, clone);
            let result = client.create_repository(&preview).unwrap();
            assert_eq!(result.project, project);
            assert_eq!(result.visibility, request.visibility);
            task.join().unwrap();
            let sent = bodies.lock().unwrap();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0]["description"], request.description);
            if github {
                assert_eq!(sent[0]["auto_init"], false);
                assert_eq!(sent[0]["private"], true);
            } else {
                assert_eq!(sent[0]["initialize_with_readme"], false);
                assert_eq!(sent[0]["namespace_id"], 42);
                assert_eq!(sent[0]["visibility"], "public");
            }
        }
    }
    #[test]
    fn hosted_repository_reviews_refuse_existing_projects_and_tampered_visibility_without_posting()
    {
        for exists in [true, false] {
            let profile = HostProfile {
                provider: "github".into(),
                origin: "https://github.com".into(),
            };
            let context = host_context(&profile).unwrap();
            let cred = Credential {
                token: "mock-secret".into(),
                login: "alice".into(),
            };
            let request = RepositoryCreateRequest {
                profile,
                namespace_id: "user:alice".into(),
                name: "demo".into(),
                description: String::new(),
                visibility: "private".into(),
            };
            let (base, bodies, task) = server(vec![
                ("GET", "/user", 200, json!({"login":"alice"})),
                ("GET", "/user/orgs?", 200, json!([])),
                (
                    "GET",
                    "/repos/alice/demo",
                    if exists { 200 } else { 404 },
                    json!({}),
                ),
            ]);
            let mut client = HostingClient::new(&context, &cred).unwrap();
            client.base = base;
            if exists {
                assert!(client.review_repository(&request).is_err());
            } else {
                let mut p = client.review_repository(&request).unwrap();
                p.request.visibility = "public".into();
                assert!(client.create_repository(&p).is_err());
            }
            task.join().unwrap();
            assert!(bodies.lock().unwrap().is_empty());
        }
    }
    #[test]
    fn hosted_repository_post_with_wrong_visibility_is_uncertain_and_never_deleted_or_retried() {
        let profile = HostProfile {
            provider: "github".into(),
            origin: "https://github.com".into(),
        };
        let context = host_context(&profile).unwrap();
        let cred = Credential {
            token: "mock-secret".into(),
            login: "alice".into(),
        };
        let request = RepositoryCreateRequest {
            profile,
            namespace_id: "user:alice".into(),
            name: "demo".into(),
            description: String::new(),
            visibility: "private".into(),
        };
        let (base, bodies, task) = server(vec![
            ("GET", "/user", 200, json!({"login":"alice"})),
            ("GET", "/user/orgs?", 200, json!([])),
            ("GET", "/repos/alice/demo", 404, json!({})),
            (
                "POST",
                "/user/repos",
                201,
                json!({"full_name":"alice/demo","clone_url":"https://github.com/alice/demo.git","html_url":"https://github.com/alice/demo","private":false}),
            ),
        ]);
        let mut client = HostingClient::new(&context, &cred).unwrap();
        client.base = base;
        let p = client.review_repository(&request).unwrap();
        assert_eq!(
            client.create_repository(&p).unwrap_err().code,
            "operation_unknown"
        );
        task.join().unwrap();
        assert_eq!(bodies.lock().unwrap().len(), 1);
    }
}
