//! Explicit local tags and single-tag publication; no checkout or implicit branch push.
use crate::{
    working::{run_fetch, run_offline_write},
    GitCli, RepositoryReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};
impl GitCli {
    fn tag_object(&self, path: &Path, name: &str) -> Result<Option<String>> {
        let full = format!("refs/tags/{name}");
        if self
            .optional_text(path, &["symbolic-ref", "--quiet", &full])?
            .is_some()
        {
            return Err(AppError::new(
                "tag_request",
                "Symbolic tags require manual inspection.",
            ));
        }
        self.optional_text(path, &["rev-parse", "--verify", "--quiet", &full])
    }
    fn remote_tag(&self, path: &Path, url: &str, name: &str) -> Result<Option<String>> {
        let full = format!("refs/tags/{name}");
        let args = ["ls-remote", "--refs", "--tags", "--", url, &full];
        let output=self.execute_transport(path,&args.iter().map(OsStr::new).collect::<Vec<_>>(),65536,false,"file:http:https:ssh:git")
            .map_err(|_|AppError::new("tag_inspection","Cannot inspect the tag destination. Check access and credentials, then review again."))?;
        let response = String::from_utf8(output.bytes)
            .map_err(|_| AppError::new("tag_inspection", "Invalid remote tag response."))?;
        let mut found = None;
        for line in response.lines() {
            let (oid, reference) = line
                .split_once('\t')
                .ok_or_else(|| AppError::new("tag_inspection", "Invalid remote tag response."))?;
            if reference != full || found.is_some() {
                return Err(AppError::new(
                    "tag_inspection",
                    "Ambiguous remote tag response.",
                ));
            }
            crate::validate_oid(oid)?;
            found = Some(oid.to_owned());
        }
        Ok(found)
    }
    pub fn preview_tag(&self, path: &Path, request: &TagRequest) -> Result<TagPreview> {
        let (name, oid) = match request {
            TagRequest::Create { name, oid, .. } | TagRequest::Push { name, oid, .. } => {
                (name, oid)
            }
        };
        self.valid_name(path, name, "tags")?;
        crate::validate_oid(oid)?;
        if self.text(path, &["cat-file", "-t", oid])?.trim() != "commit" {
            return Err(AppError::new(
                "tag_request",
                "Choose an exact commit object.",
            ));
        }
        let (discovery, state) = self.write_state(path)?;
        let config = self.config_snapshot(path)?;
        let object_oid = self.tag_object(path, name)?;
        let mut preview = TagPreview {
            request: request.clone(),
            object_oid: object_oid.clone(),
            commit_oid: oid.clone(),
            annotation: None,
            tagger: None,
            destination: None,
            remote_oid: None,
            already_published: false,
            blocked_reason: None,
            token: String::new(),
        };
        let mut endpoint = None;
        match request {
            TagRequest::Create { annotation, .. } => {
                if object_oid.is_some() {
                    preview.blocked_reason =
                        Some("That tag already exists. Existing tags are never replaced.".into());
                }
                if let Some(message) = annotation {
                    if message.trim().is_empty() || message.len() > 65536 || message.contains('\0')
                    {
                        return Err(AppError::new(
                            "tag_request",
                            "Enter a nonempty annotation up to 64 KiB.",
                        ));
                    }
                    let ident = self.text(path, &["var", "GIT_COMMITTER_IDENT"])?;
                    preview.tagger = Some(
                        ident
                            .trim_end()
                            .rsplitn(3, ' ')
                            .last()
                            .unwrap_or("")
                            .to_owned(),
                    );
                    preview.annotation = Some(message.clone());
                }
            }
            TagRequest::Push { remote, .. } => {
                let object = object_oid.as_ref().ok_or_else(|| {
                    AppError::new(
                        "tag_request",
                        "The selected local tag disappeared. Refresh and review again.",
                    )
                })?;
                if self
                    .text(
                        path,
                        &[
                            "rev-parse",
                            "--verify",
                            &format!("refs/tags/{name}^{{commit}}"),
                        ],
                    )?
                    .trim()
                    != oid
                {
                    return Err(AppError::new(
                        "stale_preview",
                        "The tag moved to a different commit. Refresh and review again.",
                    ));
                }
                if self.text(path, &["cat-file", "-t", object])?.trim() == "tag" {
                    let body = self
                        .read(
                            path,
                            &["cat-file".as_ref(), "-p".as_ref(), object.as_ref()],
                            131072,
                        )?
                        .bytes;
                    let body = String::from_utf8(body).map_err(|_| {
                        AppError::new("tag_request", "This tag annotation is not UTF-8.")
                    })?;
                    preview.annotation = body
                        .split_once("\n\n")
                        .map(|(_, message)| message.to_owned());
                }
                let url = self.push_destination(path, remote)?;
                if self
                    .optional_text(
                        path,
                        &[
                            "config",
                            "--bool",
                            "--get",
                            &format!("remote.{remote}.mirror"),
                        ],
                    )?
                    .as_deref()
                    == Some("true")
                    || self
                        .optional_text(path, &["config", "--get-all", "push.pushOption"])?
                        .is_some_and(|v| !v.is_empty())
                {
                    preview.blocked_reason = Some(
                        "Mirror remotes and configured push options need separate review.".into(),
                    );
                }
                let local = if let Some(file) = url.strip_prefix("file://") {
                    if file.contains('%') {
                        return Err(AppError::new(
                            "tag_request",
                            "Encoded file URLs require a local path remote for tag publication.",
                        ));
                    }
                    Some(Path::new(file).to_path_buf())
                } else {
                    let p = path.join(&url);
                    p.exists().then_some(p)
                };
                if local
                    .and_then(|p| self.discover(&p).ok())
                    .is_some_and(|d| d.common_dir == discovery.common_dir)
                {
                    preview.blocked_reason =
                        Some("Choose an independent remote repository for tag publication.".into());
                }
                preview.remote_oid = self.remote_tag(path, &url, name)?;
                preview.already_published = preview.remote_oid.as_ref() == Some(object);
                if preview.remote_oid.is_some() && !preview.already_published {
                    preview.blocked_reason=Some("The remote tag already points to a different object. It will not be overwritten.".into());
                }
                preview.destination = Some(crate::push::display_destination(&url));
                endpoint = Some(url);
            }
        }
        let data = serde_json::to_vec(&(&preview, &state, &config, &endpoint))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        preview.token = format!("{:x}", Sha256::digest(data));
        Ok(preview)
    }
    pub fn apply_tag(&self, path: &Path, request: &TagRequest, token: &str) -> Result<()> {
        let review = self.preview_tag(path, request)?;
        if review.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Tag, destination or configuration changed. Review the tag again.",
            ));
        }
        if let Some(reason) = review.blocked_reason {
            return Err(AppError::new("tag_blocked", reason));
        }
        let before = self.head_index_snapshot(path)?;
        let (name, oid) = match request {
            TagRequest::Create { name, oid, .. } | TagRequest::Push { name, oid, .. } => {
                (name, oid)
            }
        };
        match request {
            TagRequest::Create { annotation, .. } => {
                let mut args: Vec<OsString> = vec!["tag".into(), "--no-sign".into()];
                if let Some(message) = annotation {
                    args.extend([
                        "--annotate".into(),
                        "--cleanup=verbatim".into(),
                        "--message".into(),
                        message.into(),
                    ]);
                }
                args.extend(["--".into(), name.into(), oid.into()]);
                run_offline_write(path, &args)?;
                let actual = self.tag_object(path, name)?.ok_or_else(|| {
                    AppError::new(
                        "operation_unknown",
                        "The created tag disappeared. Inspect it before retrying.",
                    )
                })?;
                if let Some(message) = annotation {
                    let body = self
                        .read(
                            path,
                            &["cat-file".as_ref(), "-p".as_ref(), actual.as_ref()],
                            131072,
                        )?
                        .bytes;
                    let body = String::from_utf8(body).map_err(|_| {
                        AppError::new(
                            "operation_unknown",
                            "The created annotation could not be verified.",
                        )
                    })?;
                    let expected = message;
                    if body.split_once("\n\n").map(|(_, text)| text) != Some(expected.as_str()) {
                        return Err(AppError::new(
                            "operation_unknown",
                            "The created tag annotation differs. Inspect the tag before retrying.",
                        ));
                    }
                } else if actual != *oid {
                    return Err(AppError::new(
                        "operation_unknown",
                        "The created lightweight tag differs. Inspect it before retrying.",
                    ));
                }

                if self
                    .text(
                        path,
                        &[
                            "rev-parse",
                            "--verify",
                            &format!("refs/tags/{name}^{{commit}}"),
                        ],
                    )?
                    .trim()
                    != oid
                {
                    return Err(AppError::new(
                        "operation_unknown",
                        "Tag creation returned, but the tag differs. Inspect it before retrying.",
                    ));
                }
            }
            TagRequest::Push { remote, .. } => {
                if !review.already_published {
                    let url = self.push_destination(path, remote)?;
                    let spec = format!("{}:refs/tags/{name}", review.object_oid.as_ref().unwrap());
                    let args = [
                        "push",
                        "--porcelain",
                        "--no-force",
                        "--no-force-with-lease",
                        "--no-mirror",
                        "--no-follow-tags",
                        "--recurse-submodules=no",
                        "--",
                        &url,
                        &spec,
                    ]
                    .map(OsString::from);
                    run_fetch(path,&args).map_err(|e|AppError::new("operation_unknown",format!("Tag publication could not be confirmed ({}). Inspect the remote before retrying; the local tag is retained.",e.code)))?;
                    if self.remote_tag(path, &url, name)? != review.object_oid {
                        return Err(AppError::new("operation_unknown","Remote tag verification differs. Inspect the destination before retrying."));
                    }
                }
                if self.tag_object(path, name)? != review.object_oid {
                    return Err(AppError::new("operation_unknown","The remote contains the reviewed tag, but the local tag changed. Inspect both before retrying."));
                }
            }
        }
        if self.head_index_snapshot(path)? != before {
            return Err(AppError::new("operation_unknown","Tag operation completed, but checkout or staging changed. Inspect both results before retrying."));
        }
        Ok(())
    }
}
