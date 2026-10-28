//! Explicit empty hosted repository creation; no local Git configuration or push.
use super::*;
fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn namespace(value: &str) -> bool {
    value.len() <= 1024 && value.split('/').all(slug)
}
fn token(
    request: &RepositoryCreateRequest,
    account: &str,
    project: &str,
    url: &str,
) -> Result<String> {
    let bytes = serde_json::to_vec(&(request, account, project, url))
        .map_err(|_| err("Could not encode repository review."))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
impl HostingClient<'_> {
    pub fn repository_options(&self) -> Result<RepositoryCreateOptions> {
        let account = self.login()?;
        if account != self.credential.login {
            return Err(err("The provider account changed. Reconnect in Settings."));
        }
        let github = self.context.provider == "github";
        let rows = self
            .call(
                Method::GET,
                if github { "/user/orgs" } else { "/namespaces" },
                &[("per_page", "100".into())],
                None,
            )?
            .ok_or_else(|| err("Repository namespaces are unavailable."))?;
        let rows = rows
            .as_array()
            .ok_or_else(|| err("Invalid namespace response."))?;
        let mut namespaces = if github {
            vec![RepositoryNamespace {
                id: format!("user:{account}"),
                path: account.clone(),
            }]
        } else {
            vec![]
        };
        for row in rows.iter().take(100) {
            let path = row
                .get(if github { "login" } else { "full_path" })
                .and_then(Value::as_str)
                .filter(|v| namespace(v))
                .ok_or_else(|| err("Invalid namespace identity."))?;
            let id = if github {
                format!("org:{path}")
            } else {
                row.get("id")
                    .and_then(Value::as_u64)
                    .filter(|v| *v > 0)
                    .ok_or_else(|| err("Invalid namespace ID."))?
                    .to_string()
            };
            namespaces.push(RepositoryNamespace {
                id,
                path: path.into(),
            });
        }
        Ok(RepositoryCreateOptions {
            account,
            namespaces,
            partial: rows.len() >= 100,
        })
    }
    pub fn review_repository(
        &self,
        request: &RepositoryCreateRequest,
    ) -> Result<RepositoryCreatePreview> {
        let host = host_context(&request.profile)?;
        if host.origin != self.context.origin
            || host.provider != self.context.provider
            || !slug(&request.name)
            || request.name.ends_with(".git")
            || request.description.len() > 4096
            || request.description.chars().any(|c| c == '\0')
            || !matches!(request.visibility.as_str(), "public" | "private")
        {
            return Err(err(
                "Choose a valid repository name, visibility and connected host.",
            ));
        }
        let options = self.repository_options()?;
        let owner = options
            .namespaces
            .iter()
            .find(|v| v.id == request.namespace_id)
            .ok_or_else(|| err("Choose an available owner or namespace."))?;
        let project = format!("{}/{}", owner.path, request.name);
        if self
            .call(Method::GET, &self.repository_endpoint(&project), &[], None)?
            .is_some()
        {
            return Err(err(
                "This hosted repository already exists. Use its clone URL instead.",
            ));
        }
        let clone_url = format!("{}/{project}.git", host.origin);
        Ok(RepositoryCreatePreview {
            request: request.clone(),
            account: options.account.clone(),
            project: project.clone(),
            clone_url: clone_url.clone(),
            token: token(request, &options.account, &project, &clone_url)?,
            created: None,
        })
    }
    fn repository_endpoint(&self, project: &str) -> String {
        if self.context.provider == "github" {
            format!(
                "/repos/{}",
                project.split('/').map(encode).collect::<Vec<_>>().join("/")
            )
        } else {
            format!("/projects/{}", encode(project))
        }
    }
    pub fn create_repository(&self, p: &RepositoryCreatePreview) -> Result<CreatedRepository> {
        let host = host_context(&p.request.profile)?;
        if host.origin != self.context.origin
            || host.provider != self.context.provider
            || p.account != self.credential.login
            || p.token != token(&p.request, &p.account, &p.project, &p.clone_url)?
        {
            return Err(err("Repository review changed. Review again."));
        }
        let github = self.context.provider == "github";
        let endpoint = if github {
            if p.request.namespace_id == format!("user:{}", p.account) {
                "/user/repos".into()
            } else if let Some(owner) = p
                .request
                .namespace_id
                .strip_prefix("org:")
                .filter(|v| slug(v))
            {
                format!("/orgs/{}/repos", encode(owner))
            } else {
                return Err(err("Invalid repository owner."));
            }
        } else {
            "/projects".into()
        };
        let body = if github {
            json!({"name":p.request.name,"description":p.request.description,"private":p.request.visibility=="private","auto_init":false})
        } else {
            json!({"name":p.request.name,"path":p.request.name,"description":p.request.description,"visibility":p.request.visibility,"namespace_id":p.request.namespace_id.parse::<u64>().map_err(|_|err("Invalid namespace ID."))?,"initialize_with_readme":false})
        };
        let response = self
            .call(Method::POST, &endpoint, &[], Some(body))?
            .ok_or_else(|| {
                AppError::new(
                    "operation_unknown",
                    "Repository creation was not confirmed. Inspect the host before retrying.",
                )
            })?;
        let parse = |v: &Value| -> Result<CreatedRepository> {
            let project = v
                .get(if github {
                    "full_name"
                } else {
                    "path_with_namespace"
                })
                .and_then(Value::as_str);
            let visibility = if github {
                v.get("private")
                    .and_then(Value::as_bool)
                    .map(|v| if v { "private" } else { "public" })
            } else {
                v.get("visibility").and_then(Value::as_str)
            };
            let clone = v
                .get(if github {
                    "clone_url"
                } else {
                    "http_url_to_repo"
                })
                .and_then(Value::as_str);
            let web = v
                .get(if github { "html_url" } else { "web_url" })
                .and_then(Value::as_str);
            let expected_web = format!("{}/{}", host.origin, p.project);
            if project != Some(p.project.as_str())
                || visibility != Some(p.request.visibility.as_str())
                || clone != Some(p.clone_url.as_str())
                || web != Some(expected_web.as_str())
            {
                return Err(AppError::new("operation_unknown","Created repository identity or visibility differs from the review. Inspect the host; no push or deletion was attempted."));
            }
            Ok(CreatedRepository {
                project: p.project.clone(),
                clone_url: p.clone_url.clone(),
                web_url: expected_web,
                visibility: p.request.visibility.clone(),
            })
        };
        let result = parse(&response)?;
        let verified=self.call(Method::GET,&self.repository_endpoint(&p.project),&[],None).map_err(|_|AppError::new("operation_unknown","Repository may have been created. Inspect the reviewed host and path before retrying."))?.ok_or_else(||AppError::new("operation_unknown","Created repository could not be verified. Inspect the host before retrying."))?;
        parse(&verified)?;
        let endpoint = format!(
            "{}{}",
            self.repository_endpoint(&p.project),
            if github {
                "/branches"
            } else {
                "/repository/branches"
            }
        );
        let branches = self.call(Method::GET, &endpoint, &[("per_page", "1".into())], None)
            .map_err(|_| AppError::new("operation_unknown", "Repository created, but empty history could not be verified. Inspect the host before retrying."))?;
        if !branches
            .as_ref()
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            return Err(AppError::new("operation_unknown", "The created repository contains history or could not be verified empty. Inspect it before adding a remote or pushing."));
        }
        Ok(result)
    }
}
