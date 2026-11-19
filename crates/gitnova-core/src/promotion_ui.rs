//! Desktop review for the isolated file promotion engine used by Nova CLI.
use crate::Application;
use gitnova_domain::{
    AppError, GitCommitIdentity, HistoricalPatch, HostProfile, HostedRequest, HostingContext,
    HostingPreview, HostingRequest, HostingSelection, NativePath, Result,
};
use gitnova_git::promotion::{promotion_stamp, Promotion, PromotionRequest, SelectedFile};
use gitnova_hosting::{Credential, HostingClient};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionOptions {
    pub request: PromotionRequest,
    pub title: String,
    pub body: String,
    pub create_request: bool,
    pub profiles: Vec<HostProfile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionReview {
    token: String,
    source_oid: String,
    target_oid: String,
    destination: String,
    identity: GitCommitIdentity,
    files: Vec<SelectedFile>,
    patch: HistoricalPatch,
    hosting: Option<HostingContext>,
    hosting_error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPromotion {
    remote: String,
    branch: String,
    oid: String,
    target: String,
    target_oid: String,
    recovery: NativePath,
    hosting: Option<HostingContext>,
    title: String,
    body: String,
    scope: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PromotionAction {
    Preview {
        options: PromotionOptions,
    },
    Publish {
        options: PromotionOptions,
        token: String,
    },
    ReviewRequest {
        published: Box<PublishedPromotion>,
    },
    CreateRequest {
        published: Box<PublishedPromotion>,
        token: String,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum PromotionResult {
    Review(Box<PromotionReview>),
    Published(Box<PublishedPromotion>),
    RequestReview(Box<HostingPreview>),
    Request(Box<HostedRequest>),
}

fn err(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}

fn profile_for_remote<'a>(
    profiles: &'a [HostProfile],
    url: &str,
) -> Result<Option<&'a HostProfile>> {
    if profiles.len() > 20 {
        return Err(err("hosting", "At most 20 host connections are supported."));
    }
    let mut matched = Vec::new();
    for profile in profiles {
        if gitnova_hosting::matches_host(profile, url)? {
            matched.push(profile);
        }
    }
    matched.sort_by_key(|profile| std::cmp::Reverse(profile.origin.len()));
    match matched.as_slice() {
        [] => Ok(None),
        [profile] => Ok(Some(profile)),
        [first, second, ..]
            if url.starts_with("https://")
                && first.origin.trim_end_matches('/') != second.origin.trim_end_matches('/') =>
        {
            Ok(Some(first))
        }
        _ => Err(err(
            "hosting",
            "More than one connection matches this remote.",
        )),
    }
}

fn promotion_context(
    request: &PromotionRequest,
    profile: Option<&HostProfile>,
    url: &str,
) -> Result<HostingContext> {
    let selection = HostingSelection {
        remote: request.remote.clone(),
        branch: request.branch.clone(),
        remote_branch: None,
        oid: String::new(),
        provider: profile.map(|p| p.provider.clone()),
        origin: profile.map(|p| p.origin.clone()),
    };
    let mut context = gitnova_hosting::resolve(&selection, url).map_err(|e| {
        if e.message == "Choose GitHub or GitLab for this host." {
            err(
                "hosting",
                "Connect this remote in Settings → Connections, or choose Publish branch only.",
            )
        } else {
            e
        }
    })?;
    let credential = gitnova_hosting::credential(&context)?;
    context.account = Some(credential.login.clone());
    HostingClient::new(&context, &credential)?;
    Ok(context)
}

impl Application {
    fn prepare_promotion_review(
        &self,
        membership: &str,
        options: &PromotionOptions,
    ) -> Result<(Promotion, Option<HostingContext>, Option<String>, String)> {
        if options.title.trim().is_empty()
            || options.title.len() > 1024
            || options.title.contains('\0')
            || options.body.len() > 65536
        {
            return Err(err(
                "promotion",
                "Enter a title up to 1 KiB and a description up to 64 KiB.",
            ));
        }
        let root = self.path(membership)?;
        let url = self
            .git
            .promotion_destination(&root, &options.request.remote)?;
        let profile = profile_for_remote(&options.profiles, &url);
        let identity = match self.git.repository_commit_identity(&root)? {
            Some(identity) => Some(identity),
            None => match profile.as_ref().ok().and_then(|v| *v) {
                Some(profile) => self.connection_commit_identity(profile)?,
                None => None,
            },
        };
        let promotion =
            self.git
                .prepare_promotion_with_identity(&root, options.request.clone(), identity)?;
        if promotion.url != url {
            return Err(err(
                "stale_preview",
                "Remote configuration or branch tips changed. Run the preview again.",
            ));
        }
        let (hosting, hosting_error) = if options.create_request {
            match profile.and_then(|value| promotion_context(&options.request, value, &url)) {
                Ok(context) => (Some(context), None),
                Err(error) => (None, Some(error.message)),
            }
        } else {
            (None, None)
        };
        let token = promotion_stamp(&(
            membership,
            NativePath::new(&root),
            options,
            &promotion.url,
            &promotion.source_oid,
            &promotion.target_oid,
            &promotion.identity,
            &hosting,
            &hosting_error,
        ))?;
        Ok((promotion, hosting, hosting_error, token))
    }

    fn published_request(
        &self,
        membership: &str,
        published: &PublishedPromotion,
    ) -> Result<(HostingContext, Credential, HostingPreview)> {
        let saved = published
            .hosting
            .as_ref()
            .ok_or_else(|| err("hosting", "This branch was published without a request."))?;
        let root = self.path(membership)?;
        let url = self.git.promotion_destination(&root, &published.remote)?;
        let mut context = gitnova_hosting::resolve(&saved.selection, &url)?;
        let credential = gitnova_hosting::credential(&context)?;
        context.account = Some(credential.login.clone());
        let scope = promotion_stamp(&(membership, NativePath::new(&root), &url, &context))?;
        if scope != published.scope {
            return Err(err("stale_preview", "The remote or hosting account changed. Inspect the published branch before creating a request."));
        }
        let mut selection = context.selection.clone();
        selection.oid = published.oid.clone();
        let request = HostingRequest {
            selection,
            target_project: context.project.clone(),
            target_branch: published.target.clone(),
            title: published.title.clone(),
            body: published.body.clone(),
            draft: false,
        };
        let review = HostingClient::new(&context, &credential)?.review(&request, &scope)?;
        if review.source_oid.as_deref() != Some(&published.oid)
            || review.target_oid.as_deref() != Some(&published.target_oid)
        {
            return Err(err("stale_preview", "The review branch or destination moved after publication. Review the comparison on the provider."));
        }
        Ok((context, credential, review))
    }

    pub fn promotion(&self, membership: &str, action: PromotionAction) -> Result<PromotionResult> {
        match action {
            PromotionAction::Preview { options } => {
                let (promotion, hosting, hosting_error, token) =
                    self.prepare_promotion_review(membership, &options)?;
                let patch = promotion.patch()?;
                Ok(PromotionResult::Review(Box::new(PromotionReview {
                    token,
                    patch,
                    hosting,
                    hosting_error,
                    source_oid: promotion.source_oid,
                    target_oid: promotion.target_oid,
                    destination: gitnova_git::push::display_destination(&promotion.url),
                    identity: promotion.identity,
                    files: promotion.files,
                })))
            }
            PromotionAction::Publish { options, token } => {
                let root = self.path(membership)?;
                let _lock = self.git.acquire_operation(&root)?;
                let (promotion, hosting, hosting_error, fresh) =
                    self.prepare_promotion_review(membership, &options)?;
                if token != fresh {
                    return Err(err(
                        "stale_preview",
                        "Remote configuration or branch tips changed. Run the preview again.",
                    ));
                }
                if let Some(reason) = hosting_error {
                    return Err(AppError::new("hosting", reason));
                }
                let scope = promotion_stamp(&(
                    membership,
                    NativePath::new(&root),
                    &promotion.url,
                    hosting.as_ref(),
                ))?;
                let target_oid = promotion.target_oid.clone();
                let mut recovery = None;
                let oid = promotion.publish(&options.title, |path| {
                    recovery = Some(NativePath::new(path));
                    Ok(())
                }).map_err(|error| match &recovery {
                    Some(path) => AppError::new("operation_unknown", format!(
                        "{} {} {}", error.message,
                        "Publication could not be verified. Inspect the remote before retrying. Recovery repository:",
                        path.display,
                    )),
                    None => error,
                })?;
                let recovery = recovery.ok_or_else(|| {
                    err("operation_unknown", "Publication could not be verified.")
                })?;
                Ok(PromotionResult::Published(Box::new(PublishedPromotion {
                    remote: options.request.remote,
                    branch: options.request.branch,
                    target: options.request.target,
                    oid,
                    target_oid,
                    recovery,
                    hosting,
                    scope,
                    title: options.title,
                    body: options.body,
                })))
            }
            PromotionAction::ReviewRequest { published } => {
                let (_, _, review) = self.published_request(membership, &published)?;
                Ok(PromotionResult::RequestReview(Box::new(review)))
            }
            PromotionAction::CreateRequest { published, token } => {
                let _lock = self.git.acquire_operation(&self.path(membership)?)?;
                let (context, credential, review) =
                    self.published_request(membership, &published)?;
                if token != review.token {
                    return Err(err("stale_preview", "Source, target, account or configuration changed. Review the request again."));
                }
                let result = HostingClient::new(&context, &credential)?.create(&review)?;
                Ok(PromotionResult::Request(Box::new(result)))
            }
        }
    }
}
