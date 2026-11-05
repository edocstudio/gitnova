//! One bounded repository-level list, not a request per visible commit.
use super::{encode, err, project_id, value_id, value_string, HostingClient};
use gitnova_domain::{BranchRequests, OpenBranchRequest, Result};
use reqwest::Method;
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};
impl HostingClient<'_> {
    pub fn open_branch_requests(&self) -> Result<BranchRequests> {
        let project = self.project(&self.context.project)?;
        let github = self.context.provider == "github";
        let actual = value_string(
            &project,
            if github {
                "full_name"
            } else {
                "path_with_namespace"
            },
        )
        .ok_or_else(|| err("Project identity is unavailable."))?;
        if !(if github {
            actual.eq_ignore_ascii_case(&self.context.project)
        } else {
            actual == self.context.project
        }) {
            return Err(err("The provider returned a different repository."));
        }
        let identity = project_id(&project)?;
        let endpoint = self.project_endpoint(&project)?;
        let mut result = BranchRequests {
            provider: self.context.provider.clone(),
            origin: self.context.origin.clone(),
            project: actual.clone(),
            project_id: identity.clone(),
            remote: self.context.selection.remote.clone(),
            requests: Vec::new(),
            partial: false,
            checked_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let mut seen = HashSet::new();
        for page in 1..=3 {
            let mut query = vec![
                ("state", if github { "open" } else { "opened" }.into()),
                ("per_page", "100".into()),
                ("page", page.to_string()),
            ];
            if github {
                query.extend([("sort", "updated".into()), ("direction", "desc".into())]);
            } else {
                query.extend([
                    ("scope", "all".into()),
                    ("order_by", "updated_at".into()),
                    ("sort", "desc".into()),
                ]);
            }
            let value = self
                .call(
                    Method::GET,
                    &format!(
                        "{endpoint}/{}",
                        if github { "pulls" } else { "merge_requests" }
                    ),
                    &query,
                    None,
                )?
                .ok_or_else(|| err("Open requests could not be read for this repository."))?;
            let rows = value
                .as_array()
                .filter(|v| v.len() <= 100)
                .ok_or_else(|| err("Invalid open request response."))?;
            for row in rows {
                let target_id = if github {
                    value_id(&row["base"]["repo"]["id"])?
                } else {
                    value_id(&row["target_project_id"])?
                };
                if target_id != identity {
                    return Err(err(
                        "The provider returned a request for a different target repository.",
                    ));
                }
                let request = self.parse_request(row)?;
                if request.state == "unknown" {
                    return Err(err("The provider returned an unknown request state."));
                }
                if !matches!(request.state.as_str(), "open" | "opened") {
                    continue;
                }
                let source_id = if github {
                    &row["head"]["repo"]["id"]
                } else {
                    &row["source_project_id"]
                };
                let source_id = if source_id.is_null() {
                    None
                } else {
                    Some(value_id(source_id)?)
                };
                if [
                    request.source_branch.as_str(),
                    request.target_branch.as_str(),
                ]
                .iter()
                .any(|s| s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
                    || request.number == 0
                    || request.title.len() > 4096
                {
                    return Err(err("Invalid open request branch identity."));
                }
                // Derive the path from the validated target project, never the source's folder name.
                let project_path = actual.split('/').map(encode).collect::<Vec<_>>().join("/");
                let expected = format!(
                    "{}/{project_path}/{}{}",
                    self.context.origin,
                    if github { "pull/" } else { "-/merge_requests/" },
                    request.number
                );
                if request.url != expected {
                    return Err(err(
                        "The provider returned a request URL for a different repository.",
                    ));
                }
                if seen.insert(request.number) {
                    result.requests.push(OpenBranchRequest {
                        number: request.number,
                        title: request.title,
                        draft: request.draft,
                        url: request.url,
                        source_branch: request.source_branch,
                        target_branch: request.target_branch,
                        source_project_id: source_id,
                    });
                }
            }
            if rows.len() < 100 {
                break;
            }
            if page == 3 {
                result.partial = true;
            }
        }
        Ok(result)
    }
}
