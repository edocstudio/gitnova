use super::*;
impl Application {
    pub fn host_account(&self, profile: &HostProfile) -> Result<HostingContext> {
        let mut context = gitnova_hosting::host_context(profile)?;
        context.account = gitnova_hosting::credential(&context).ok().map(|c| c.login);
        Ok(context)
    }
    pub fn connect_host(
        &self,
        profile: &HostProfile,
        prompt: &CredentialPrompt,
    ) -> Result<HostingContext> {
        let mut context = gitnova_hosting::host_context(profile)?;
        context.account = Some(gitnova_hosting::connect(&context, prompt)?);
        Ok(context)
    }
    pub fn disconnect_host(&self, profile: &HostProfile) -> Result<()> {
        gitnova_hosting::disconnect(&gitnova_hosting::host_context(profile)?)
    }
    pub fn ci_connection(
        &self,
        id: &str,
        oid: &str,
        profiles: &[HostProfile],
    ) -> Result<Option<HostingSelection>> {
        if profiles.len() > 20 {
            return Err(AppError::new(
                "hosting",
                "At most 20 host connections are supported.",
            ));
        }
        for profile in profiles {
            gitnova_hosting::host_context(profile)?;
        }
        let path = self.path(id)?;
        let mut remotes = self.git.remote_branches(&path)?.remotes;
        remotes.sort_by_key(|r| if r == "origin" { 0 } else { 1 });
        let mut candidates = Vec::new();
        for remote in remotes.into_iter().take(20) {
            let url = self.git.pipeline_remote(&path, &remote, oid)?;
            let mut matches = profiles
                .iter()
                .filter(|p| gitnova_hosting::matches_host(p, &url).unwrap_or(false))
                .collect::<Vec<_>>();
            matches.sort_by_key(|p| std::cmp::Reverse(p.origin.len()));
            if matches.len() > 1
                && (!url.starts_with("https://")
                    || matches[0].origin.trim_end_matches('/')
                        == matches[1].origin.trim_end_matches('/'))
            {
                return Err(AppError::new(
                    "hosting",
                    "More than one connection matches this remote.",
                ));
            }
            if let Some(p) = matches.first() {
                let context = gitnova_hosting::resolve(
                    &HostingSelection {
                        remote: remote.clone(),
                        branch: String::new(),
                        remote_branch: None,
                        oid: oid.into(),
                        provider: Some(p.provider.clone()),
                        origin: Some(p.origin.clone()),
                    },
                    &url,
                )?;
                if remote == "origin" {
                    return Ok(Some(context.selection));
                }
                candidates.push(context.selection);
            }
        }
        if candidates.len() > 1 {
            return Err(AppError::new(
                "hosting",
                "More than one connection matches this remote.",
            ));
        }
        Ok(candidates.pop())
    }
    pub fn commit_link(&self, membership_id: &str, selection: &HostingSelection) -> Result<String> {
        let url = self.git.pipeline_remote(
            &self.path(membership_id)?,
            &selection.remote,
            &selection.oid,
        )?;
        let context = gitnova_hosting::resolve(selection, &url)?;
        gitnova_hosting::commit_url(&context, &selection.oid)
    }

    pub fn pipeline_context(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
    ) -> Result<HostingContext> {
        let url = self.git.pipeline_remote(
            &self.path(membership_id)?,
            &selection.remote,
            &selection.oid,
        )?;
        let mut context = gitnova_hosting::resolve(selection, &url)?;
        context.account = gitnova_hosting::credential(&context).ok().map(|c| c.login);
        Ok(context)
    }
    pub fn connect_pipeline(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
        prompt: &CredentialPrompt,
    ) -> Result<HostingContext> {
        let mut context = self.pipeline_context(membership_id, selection)?;
        context.account = Some(gitnova_hosting::connect(&context, prompt)?);
        Ok(context)
    }
    pub fn pipelines(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
        refresh: bool,
    ) -> Result<PipelineSnapshot> {
        self.pipeline_read(membership_id, selection, refresh, false)
    }
    pub fn ci_summary(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
    ) -> Result<PipelineSnapshot> {
        self.pipeline_read(membership_id, selection, false, true)
    }
    fn pipeline_read(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
        refresh: bool,
        summary: bool,
    ) -> Result<PipelineSnapshot> {
        if summary {
            let remote = self.git.pipeline_remote(
                &self.path(membership_id)?,
                &selection.remote,
                &selection.oid,
            )?;
            let profile = HostProfile {
                provider: selection
                    .provider
                    .clone()
                    .ok_or_else(|| AppError::new("hosting", "Choose GitHub or GitLab."))?,
                origin: selection
                    .origin
                    .clone()
                    .ok_or_else(|| AppError::new("hosting", "Invalid host."))?,
            };
            if !gitnova_hosting::matches_host(&profile, &remote)? {
                return Err(AppError::new(
                    "hosting",
                    "The Git remote no longer matches this host connection.",
                ));
            }
        }
        let context = self.pipeline_context(membership_id, selection)?;
        let credential = gitnova_hosting::credential(&context)?;
        let key = serde_json::to_string(&(
            summary,
            &context.provider,
            &context.origin,
            &context.project,
            &credential.login,
            &selection.oid,
        ))
        .map_err(|_| AppError::new("hosting", "Could not identify pipeline scope."))?;
        // Serialize CI reads and coalesce identical requests, without locking the Git catalog.
        let mut cache = self
            .pipeline_cache
            .try_lock()
            .map_err(|_| AppError::new("hosting", "Pipeline cache unavailable."))?;
        if let Some((_, when, snapshot)) = cache.iter().find(|(k, _, _)| k == &key) {
            let active = snapshot.runs.iter().any(|r| {
                matches!(
                    r.status.as_str(),
                    "running" | "pending" | "created" | "preparing" | "waiting_for_resource"
                )
            });
            if when.elapsed().as_secs() < 2
                || !refresh && when.elapsed().as_secs() < if active { 15 } else { 60 }
            {
                return Ok(snapshot.clone());
            }
        }
        let client = gitnova_hosting::HostingClient::new(&context, &credential)?;
        let snapshot = if summary {
            client.ci_summary(&selection.oid)?
        } else {
            client.pipelines(&selection.oid)?
        };
        cache.retain(|(k, _, _)| k != &key);
        while cache.len() >= 128 {
            cache.pop_front();
        }
        cache.push_back((key, std::time::Instant::now(), snapshot.clone()));
        Ok(snapshot)
    }

    pub fn hosting_context(
        &self,
        selection: &HostingSelection,
        membership_id: &str,
    ) -> Result<HostingContext> {
        if let Some(remote_branch) = &selection.remote_branch {
            self.git
                .validate_hosting_branch(&self.path(membership_id)?, remote_branch)?;
        }
        let (url, _) = self.git.hosting_source(
            &self.path(membership_id)?,
            &selection.branch,
            &selection.oid,
            &selection.remote,
        )?;
        let mut context = gitnova_hosting::resolve(selection, &url)?;
        context.account = gitnova_hosting::credential(&context).ok().map(|c| c.login);
        Ok(context)
    }
    pub fn disconnect_hosting(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
    ) -> Result<HostingContext> {
        let mut context = self.hosting_context(selection, membership_id)?;
        gitnova_hosting::disconnect(&context)?;
        context.account = None;
        Ok(context)
    }
    pub fn connect_hosting(
        &self,
        membership_id: &str,
        selection: &HostingSelection,
        prompt: &CredentialPrompt,
    ) -> Result<HostingContext> {
        let mut context = self.hosting_context(selection, membership_id)?;
        context.account = Some(gitnova_hosting::connect(&context, prompt)?);
        Ok(context)
    }
    pub fn preview_hosting(
        &self,
        membership_id: &str,
        request: &HostingRequest,
    ) -> Result<HostingPreview> {
        let context = self.hosting_context(&request.selection, membership_id)?;
        let credential = gitnova_hosting::credential(&context)?;
        let (_, fingerprint) = self.git.hosting_source(
            &self.path(membership_id)?,
            &request.selection.branch,
            &request.selection.oid,
            &request.selection.remote,
        )?;
        gitnova_hosting::HostingClient::new(&context, &credential)?.review(request, &fingerprint)
    }
    pub fn create_hosting(
        &self,
        membership_id: &str,
        request: &HostingRequest,
        token: &str,
    ) -> Result<HostingOutcome> {
        let preview = self.preview_hosting(membership_id, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Source, target, account or configuration changed. Review the request again.",
            ));
        }
        if preview.existing.is_none() {
            if let Some(reason) = &preview.blocked_reason {
                return Err(AppError::new("hosting", reason));
            }
        }
        let mut change_request = None;
        let mut created = false;
        let mut receipt=self.execute_write_detailed(membership_id,"create_change_request",None,None,Some(PlannedOperation::Hosting(Box::new(preview.clone()))),|_,_|{
            // Recheck under the repository operation guard; never replay a POST automatically.
            let current=self.preview_hosting(membership_id,request)?;
            if current.token!=token{return Err(AppError::new("stale_preview","Source, target, account or configuration changed. Review the request again."));}
            if let Some(existing)=current.existing{change_request=Some(existing);return Ok(());}
            if let Some(reason)=current.blocked_reason{return Err(AppError::new("hosting",reason));}
            let credential=gitnova_hosting::credential(&current.context)?;
            if Some(credential.login.as_str())!=current.context.account.as_deref(){return Err(AppError::new("stale_preview","The provider account changed. Review the request again."));}
            change_request=Some(gitnova_hosting::HostingClient::new(&current.context,&credential)?.create(&current)?);created=true;Ok(())
        })?;
        if let Some(value) = &change_request {
            if let Some(PlannedOperation::Hosting(p)) = &mut receipt.planned {
                p.existing = Some(value.clone());
            }
            receipt.summary = "Provider request result verified".into();
            if self.save_receipt(&receipt).is_err() {
                receipt.status = "unknown".into();
                receipt.recovery=Some("The provider result is available, but its updated receipt could not be saved. Inspect existing requests before retrying.".into());
            }
        }
        Ok(HostingOutcome {
            receipt,
            change_request,
            created,
        })
    }
}

impl Application {
    pub fn repository_create_options(
        &self,
        profile: &HostProfile,
    ) -> Result<RepositoryCreateOptions> {
        let context = gitnova_hosting::host_context(profile)?;
        let credential = gitnova_hosting::credential(&context)?;
        gitnova_hosting::HostingClient::new(&context, &credential)?.repository_options()
    }
    pub fn preview_repository_create(
        &self,
        id: &str,
        request: &RepositoryCreateRequest,
    ) -> Result<RepositoryCreatePreview> {
        self.path(id)?;
        let context = gitnova_hosting::host_context(&request.profile)?;
        let credential = gitnova_hosting::credential(&context)?;
        gitnova_hosting::HostingClient::new(&context, &credential)?.review_repository(request)
    }
    pub fn create_repository(
        &self,
        id: &str,
        request: &RepositoryCreateRequest,
        token: &str,
    ) -> Result<RepositoryCreateOutcome> {
        let p = self.preview_repository_create(id, request)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository review changed. Review again.",
            ));
        }
        let mut repository = None;
        let mut receipt = self.execute_write_detailed(
            id,
            "create_hosted_repository",
            None,
            None,
            Some(PlannedOperation::RepositoryCreate(Box::new(p))),
            |_, _| {
                let current = self.preview_repository_create(id, request)?;
                if current.token != token {
                    return Err(AppError::new(
                        "stale_preview",
                        "Repository review changed. Review again.",
                    ));
                }
                let context = gitnova_hosting::host_context(&request.profile)?;
                let credential = gitnova_hosting::credential(&context)?;
                repository = Some(
                    gitnova_hosting::HostingClient::new(&context, &credential)?
                        .create_repository(&current)?,
                );
                Ok(())
            },
        )?;
        if let Some(value) = &repository {
            if let Some(PlannedOperation::RepositoryCreate(p)) = &mut receipt.planned {
                p.created = Some(value.clone());
            }
            receipt.summary = "Empty hosted repository created and verified; no code pushed".into();
            if self.save_receipt(&receipt).is_err() {
                receipt.status = "unknown".into();
                receipt.recovery=Some("Repository created, but updated receipt could not be saved. Inspect the host before retrying.".into());
            }
        }
        Ok(RepositoryCreateOutcome {
            receipt,
            repository,
        })
    }
}
