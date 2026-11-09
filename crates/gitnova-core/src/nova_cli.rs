//! Interactive CLI orchestration. Preview is the default; publication and requests are separate steps.
use gitnova_domain::{AppError, GitCommitIdentity, HostingRequest, HostingSelection, Result};
use gitnova_git::{promotion::PromotionRequest, GitCli};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::Path,
};

pub const HELP: &str = "GitNova — selected file promotion\n\n  nova promote --source SOURCE --target TARGET --branch NEW_BRANCH [options] -- FILE...\n\nReads source and target from the selected remote (default: origin).\nCopies whole file snapshots onto target; other files retain the target version.\nYour current branch, index and working files stay in place, including dirty work.\n\nOptions:\n  --execute             Review, then type confirmation to publish and create PR/MR\n  --remote NAME         Remote used for both reading and publication\n  --provider PROVIDER   github or gitlab (required for unknown self-managed hosts)\n  --host HTTPS_ORIGIN   Optional provider web origin, including a base path\n  --title TEXT          Commit and request title\n  --description TEXT    Request description\n  --allow-deletions     Permit explicitly selected files absent from source\n  --no-request          Publish only; do not create a PR/MR\n\nExample:\n  nova promote --source develop --target release --branch promote/config -- config/app.yaml\n\nRun again with --execute to confirm. This does not merge either branch.\nIdentity uses repository Git settings, then matching connection defaults in the embedded terminal, then Git configuration. It is displayed before publication.\nIsolated commits do not run the original repository's hooks or checkout filters.\n";

#[derive(Debug)]
struct Options {
    request: PromotionRequest,
    execute: bool,
    no_request: bool,
    provider: Option<String>,
    host: Option<String>,
    title: String,
    body: String,
}
fn error(message: impl Into<String>) -> AppError {
    AppError::new("nova_cli", message)
}
fn parse(args: &[String]) -> Result<Options> {
    if args.first().map(String::as_str) != Some("promote") {
        return Err(error("Expected nova promote; run nova --help."));
    }
    let mut values = BTreeMap::new();
    let mut flags = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            files.extend_from_slice(&args[index + 1..]);
            break;
        }
        if ["--execute", "--no-request", "--allow-deletions"].contains(&arg.as_str()) {
            if !flags.insert(arg.clone()) {
                return Err(error("Duplicate option."));
            }
        } else if [
            "--source",
            "--target",
            "--branch",
            "--remote",
            "--provider",
            "--host",
            "--title",
            "--description",
        ]
        .contains(&arg.as_str())
        {
            index += 1;
            let value = args
                .get(index)
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| error(format!("Missing value for {arg}.")))?;
            if value.len() > 65536 || values.insert(arg.clone(), value.clone()).is_some() {
                return Err(error("Duplicate or oversized option."));
            }
        } else {
            return Err(error(format!(
                "Unknown option {arg}; put exact file paths after --."
            )));
        }
        index += 1;
    }
    let required = |name: &str| {
        values
            .get(name)
            .cloned()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| error(format!("Required: {name}")))
    };
    let source = required("--source")?;
    let target = required("--target")?;
    let title = values
        .get("--title")
        .cloned()
        .unwrap_or_else(|| format!("Promote selected files: {source} -> {target}"));
    if title.trim().is_empty() || title.len() > 1024 || title.chars().any(char::is_control) {
        return Err(error("Enter a one-line title up to 1 KiB."));
    }
    let provider = values.get("--provider").cloned();
    if provider
        .as_deref()
        .is_some_and(|v| !["github", "gitlab"].contains(&v))
    {
        return Err(error("Provider must be github or gitlab."));
    }
    Ok(Options {
        request: PromotionRequest {
            remote: values
                .get("--remote")
                .cloned()
                .unwrap_or_else(|| "origin".into()),
            source,
            target,
            branch: required("--branch")?,
            files,
            allow_deletions: flags.contains("--allow-deletions"),
        },
        execute: flags.contains("--execute"),
        no_request: flags.contains("--no-request"),
        provider,
        host: values.get("--host").cloned(),
        title,
        body: values.get("--description").cloned().unwrap_or_default(),
    })
}
fn confirm(input: &mut impl BufRead, output: &mut impl Write, expected: &str) -> Result<()> {
    writeln!(
        output,
        "Type {expected:?} to continue (anything else cancels):"
    )?;
    output.flush()?;
    let mut answer = String::new();
    std::io::Read::take(&mut *input, 4097).read_line(&mut answer)?;
    if answer.trim_end_matches(['\r', '\n']) != expected {
        return Err(error("Cancelled. No further steps were executed."));
    }
    Ok(())
}
fn identity(
    root: &Path,
    remote: &str,
    settings: Option<&Path>,
) -> Result<Option<GitCommitIdentity>> {
    if let Some(local) = GitCli.repository_commit_identity(root)? {
        return Ok(Some(local));
    }
    let Some(file) = settings else {
        return Ok(None);
    };
    let url = GitCli.text(root, &["remote", "get-url", "--", remote])?;
    let mut rows = crate::commit_identities::load(file)?
        .into_iter()
        .filter(|row| gitnova_hosting::matches_host(&row.profile, url.trim_end()).unwrap_or(false))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.profile.origin.len()));
    if rows.len() > 1
        && (!url.starts_with("https://") || rows[0].profile.origin == rows[1].profile.origin)
    {
        return Err(error(
            "More than one connection matches this commit identity. Review Settings first.",
        ));
    }
    Ok(rows.into_iter().next().map(|row| row.identity))
}
pub fn run(
    args: &[String],
    root: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    if args.is_empty() || args == ["--help"] || args == ["help"] {
        write!(output, "{HELP}")?;
        return Ok(());
    }
    let options = parse(args)?;
    writeln!(
        output,
        "Reading selected remote branches into an isolated preview..."
    )?;
    output.flush()?;
    let settings = std::env::var_os("GITNOVA_CLI_IDENTITIES_FILE").map(std::path::PathBuf::from);
    let identity = identity(root, &options.request.remote, settings.as_deref())?;
    let preview = GitCli.prepare_promotion_with_identity(root, options.request, identity)?;
    writeln!(
        output,
        "Remote {}: {}",
        preview.request.remote,
        gitnova_git::push::display_destination(&preview.url)
    )?;
    writeln!(output, "{} ({}) -> {} ({})\nReview branch: {} -> {}\nCommitter: {} <{}>\nWhole file snapshots; original checkout stays in place.", preview.request.source, preview.source_oid, preview.request.target, preview.target_oid, preview.request.branch, preview.request.target, preview.identity.name, preview.identity.email)?;
    for file in &preview.files {
        writeln!(output, "  {}  {}", file.action, file.path)?;
    }
    writeln!(output, "{}", preview.diff_stat)?;
    writeln!(output, "Title: {}\nDescription: {}\nOriginal repository hooks and checkout filters do not run in this isolated flow.", options.title, options.body)?;
    let selection = HostingSelection {
        remote: preview.request.remote.clone(),
        branch: preview.request.branch.clone(),
        remote_branch: None,
        oid: String::new(),
        provider: options.provider,
        origin: options.host,
    };
    let context = if options.no_request {
        None
    } else {
        Some(gitnova_hosting::resolve(&selection, &preview.url)?)
    };
    if !options.execute {
        writeln!(output, "Preview only. Add --execute to review publication and PR/MR creation. No branch or request was published.")?;
        return Ok(());
    }
    // Fail missing account/invalid provider before publishing anything.
    let credential = context
        .as_ref()
        .map(gitnova_hosting::credential)
        .transpose()?;
    if let (Some(context), Some(credential)) = (&context, &credential) {
        let _ = gitnova_hosting::HostingClient::new(context, credential)?;
    }
    let branch = preview.request.branch.clone();
    let target = preview.request.target.clone();
    let target_oid = preview.target_oid.clone();
    confirm(input, output, &format!("publish {branch}"))?;
    let published = preview.publish(&options.title, |path| {
        writeln!(output, "Retained recovery repository: {}", path.display())?;
        output.flush()?;
        Ok(())
    });
    let oid = published.map_err(|e| error(format!("{} Publication may be incomplete; inspect the remote and retained repository before retrying. No automatic retry.", e.message)))?;
    writeln!(
        output,
        "Published {branch} at {oid}. Original checkout and staging were preserved."
    )?;
    output.flush()?;
    if let (Some(mut context), Some(credential)) = (context, credential) {
        context.selection.oid = oid;
        let request = HostingRequest {
            selection: context.selection.clone(),
            target_project: context.project.clone(),
            target_branch: target.clone(),
            title: options.title,
            body: options.body,
            draft: false,
        };
        let client = gitnova_hosting::HostingClient::new(&context, &credential)?;
        let review = client.review(&request, "nova-promote")?;
        if review.target_oid.as_deref() != Some(&target_oid) {
            return Err(error("Branch was published but the destination moved. Review the provider comparison before opening a request."));
        }
        if let Some(existing) = &review.existing {
            writeln!(output, "Existing request: {}", existing.url)?;
            return Ok(());
        }
        if let Some(reason) = &review.blocked_reason {
            return Err(error(format!(
                "Branch was published, request was not created: {reason}"
            )));
        }
        writeln!(
            output,
            "Create {} request: {} / {branch} -> {target}\n{}",
            context.provider, context.project, review.request.title
        )?;
        confirm(input, output, &format!("create {branch} -> {target}"))?;
        let fresh = client.review(&request, "nova-promote")?;
        if fresh.token != review.token {
            return Err(error("Branch was published but request review changed. Inspect existing requests before creating one."));
        }
        let result = client.create(&fresh).map_err(|e| error(format!("Branch was published; request result needs inspection: {}. Do not blindly retry creation.", e.message)))?;
        writeln!(output, "Request: {}", result.url)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{confirm, identity, parse};
    #[test]
    fn identity_matches_exact_remote_host_and_repository_override() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let git = gitnova_git::GitCli;
        git.text(root, &["init", "--template=", "."]).unwrap();
        git.text(
            root,
            &[
                "remote",
                "add",
                "origin",
                "https://git.example/team/project.git",
            ],
        )
        .unwrap();
        git.text(
            root,
            &[
                "remote",
                "add",
                "other",
                "https://github.com/team/project.git",
            ],
        )
        .unwrap();
        let file = root.join("identities.json");
        std::fs::write(&file, r#"[{"profile":{"provider":"gitlab","origin":"https://git.example"},"identity":{"name":"Team User","email":"team@example.test"}},{"profile":{"provider":"github","origin":"https://github.com"},"identity":{"name":"Personal User","email":"personal@example.test"}}]"#).unwrap();
        assert_eq!(
            identity(root, "origin", Some(&file))
                .unwrap()
                .unwrap()
                .email,
            "team@example.test"
        );
        assert_eq!(
            identity(root, "other", Some(&file)).unwrap().unwrap().email,
            "personal@example.test"
        );
        assert!(identity(root, "origin", None).unwrap().is_none());
        git.text(root, &["config", "gitnova.useRepositoryIdentity", "true"])
            .unwrap();
        git.text(root, &["config", "user.name", "Repository User"])
            .unwrap();
        git.text(root, &["config", "user.email", "repo@example.test"])
            .unwrap();
        assert_eq!(
            identity(root, "origin", Some(&file))
                .unwrap()
                .unwrap()
                .email,
            "repo@example.test"
        );
    }
    #[test]
    fn requires_explicit_branches_and_exact_paths() {
        assert!(parse(&["promote".into()]).is_err());
        let args = [
            "promote",
            "--source",
            "develop",
            "--target",
            "release",
            "--branch",
            "promote/config",
            "--",
            "config/app.yaml",
        ]
        .map(str::to_owned);
        let parsed = parse(&args).unwrap();
        assert!(!parsed.execute);
        assert!(!parsed.no_request);
        assert_eq!(parsed.request.files, ["config/app.yaml"]);
        let mut duplicate = args[..7].to_vec();
        duplicate.extend(["--source".into(), "other".into()]);
        assert!(parse(&duplicate).is_err());
    }
    #[test]
    fn confirmation_is_exact_and_eof_cancels() {
        assert!(confirm(&mut &b"yes\n"[..], &mut Vec::new(), "publish release-files").is_err());
        assert!(confirm(&mut &b""[..], &mut Vec::new(), "publish release-files").is_err());
        assert!(confirm(
            &mut &b"publish release-files\n"[..],
            &mut Vec::new(),
            "publish release-files"
        )
        .is_ok());
    }
}
