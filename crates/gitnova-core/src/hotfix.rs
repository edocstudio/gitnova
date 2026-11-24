//! Local review branches for repositories whose UAT environment follows release.
use crate::Application;
use gitnova_domain::{AppError, OperationReceipt, Result};
use gitnova_git::GitCli;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    tag: String,
    base: String,
    remote: String,
    production_tag: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    tag_object: String,
    deployed_object: Option<String>,
    branch: String,
    release: String,
    saved: String,
    fix: String,
    tree: String,
    checkpoint: Option<String>,
    changes: String,
}

fn fail(message: &str) -> AppError {
    AppError::new("hotfix", message)
}

fn snapshot(git: &GitCli, root: &Path, request: &Request) -> Result<Review> {
    let read = |args: &[&str]| git.text(root, args).map(|s| s.trim_end().to_owned());
    let resolve = |name: &str| read(&["rev-parse", "--verify", "--end-of-options", name]);
    let ancestor = |a: &str, b: &str| read(&["merge-base", "--is-ancestor", a, b]);
    let tag = &request.tag;
    let remote = &request.remote;
    for name in [tag, remote]
        .into_iter()
        .chain(request.production_tag.iter())
    {
        read(&["check-ref-format", &format!("refs/tags/{name}")])?;
    }
    if !read(&["remote"])?.lines().any(|r| r == remote) {
        return Err(fail("Select a configured release remote."));
    }
    let tag_object = resolve(&format!("refs/tags/{tag}"))?;
    let base = resolve(&format!("{tag_object}^{{commit}}"))?;
    let deployed_object = request
        .production_tag
        .as_ref()
        .map(|name| resolve(&format!("refs/tags/{name}")))
        .transpose()?;
    let anchor = format!("refs/gitnova/hotfix/{remote}/{tag}");
    if base != request.base {
        return Err(fail("The production tag moved. Reopen the graph action."));
    }
    let release = resolve(&format!("refs/remotes/{remote}/release^{{commit}}"))?;
    let checkpoint_branch = format!("uat-hotfix/{remote}/{tag}");
    let (saved, fix, tree, branch, checkpoint) = if let Some(deployed) = &request.production_tag {
        let checkpoint = resolve(&format!("{anchor}/uat"))?;
        if resolve(&format!("{anchor}/base"))? != base {
            return Err(fail("The production tag moved. Reopen the graph action."));
        }
        let saved = resolve(&format!("{checkpoint}^1"))?;
        let fix = resolve(&format!("{checkpoint}^2"))?;
        if deployed == tag
            || resolve(&format!(
                "{}^{{commit}}",
                deployed_object.as_deref().unwrap_or_default()
            ))? != release
            || resolve(&format!("{release}^{{tree}}"))? != resolve(&format!("{fix}^{{tree}}"))?
            || ancestor(&saved, &release).is_err()
        {
            return Err(fail("Fetch release and select the production tag on the deployed release commit. Review intervening changes separately."));
        }
        ancestor(&base, &fix)?;
        // Give Git 2.38 an explicit common base even when release has merged other
        // ancestors of the fix. The temporary commit is never assigned a ref.
        let saved_tree = resolve(&format!("{saved}^{{tree}}"))?;
        let parked = read(&[
            "commit-tree",
            &saved_tree,
            "-p",
            &base,
            "-m",
            "Parked release tree for hotfix comparison",
        ])?;
        let tree = read(&["merge-tree", "--write-tree", &parked, &fix])
            .map_err(|e| if e.code == "git_exit_1" { fail("The hotfix conflicts with saved release work. Resolve it on a separate review branch.") } else { e })?;
        (
            saved,
            fix,
            tree,
            format!("resume-release/{remote}/{tag}"),
            Some(checkpoint),
        )
    } else {
        let fix = resolve(&format!("refs/heads/hotfix/{tag}^{{commit}}"))?;
        ancestor(&base, &fix).map_err(|_| {
            fail("The hotfix branch must descend from the selected production tag.")
        })?;
        let tree = resolve(&format!("{fix}^{{tree}}"))?;
        if tree == resolve(&format!("{base}^{{tree}}"))? {
            return Err(fail("Commit the fix in the hotfix worktree first."));
        }
        if resolve(&format!("{release}^{{tree}}"))? == tree {
            return Err(fail(
                "Release already contains the complete hotfix snapshot.",
            ));
        }
        (release.clone(), fix, tree, checkpoint_branch, None)
    };
    let changes = read(&[
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--stat",
        &release,
        &tree,
        "--",
    ])?;
    Ok(Review {
        tag_object,
        deployed_object,
        branch,
        release,
        saved,
        fix,
        tree,
        checkpoint,
        changes,
    })
}

fn apply(git: &GitCli, root: &Path, request: &Request, expected: &Review) -> Result<()> {
    let plan = snapshot(git, root, request)?;
    if &plan != expected {
        return Err(fail("The reviewed refs changed. Preview again."));
    }
    let mut args = vec!["commit-tree", &plan.tree, "-p", &plan.release];
    if plan.checkpoint.is_none() {
        args.extend(["-p", &plan.fix]);
    }
    args.extend(["-m", &plan.branch]);
    let commit = git.text(root, &args)?;
    let mut transaction = format!(
        "verify refs/remotes/{}/release {}\n",
        request.remote, plan.release
    );
    let anchor = format!("refs/gitnova/hotfix/{}/{}", request.remote, request.tag);
    transaction += &format!("verify refs/tags/{} {}\n", request.tag, plan.tag_object);
    if let (Some(tag), Some(object)) = (&request.production_tag, &plan.deployed_object) {
        transaction += &format!("verify refs/tags/{tag} {object}\n");
    }
    if let Some(checkpoint) = &plan.checkpoint {
        transaction += &format!("verify {anchor}/uat {checkpoint}\nverify {anchor}/base {}\ncreate {anchor}/resume {}\n", request.base, commit.trim());
    } else {
        transaction += &format!(
            "create {anchor}/base {}\ncreate {anchor}/uat {}\n",
            request.base,
            commit.trim()
        );
        transaction += &format!("verify refs/heads/hotfix/{} {}\n", request.tag, plan.fix);
    }
    transaction += &format!("create refs/heads/{} {}\n", plan.branch, commit.trim());
    git.read_input(
        root,
        &[OsStr::new("update-ref"), OsStr::new("--stdin")],
        transaction.as_bytes(),
        4096,
    )?;
    Ok(())
}

impl Application {
    pub fn hotfix_candidate(&self, membership: &str, request: &Request) -> Result<String> {
        let root = self.path(membership)?;
        let read = |args: &[&str]| {
            self.git
                .text(&root, args)
                .map(|value| value.trim_end().to_owned())
        };
        let resolve = |name: &str| read(&["rev-parse", "--verify", "--end-of-options", name]);
        let anchor = format!("refs/gitnova/hotfix/{}/{}", request.remote, request.tag);
        read(&["check-ref-format", &format!("{anchor}/uat")])?;
        let checkpoint = resolve(&format!("{anchor}/uat"))?;
        let base = resolve(&format!("{anchor}/base"))?;
        let release = resolve(&format!(
            "refs/remotes/{}/release^{{commit}}",
            request.remote
        ))?;
        if base != request.base
            || base != resolve(&format!("refs/tags/{}^{{commit}}", request.tag))?
            || resolve(&format!("{release}^{{tree}}"))?
                != resolve(&format!("{checkpoint}^{{tree}}"))?
            || read(&[
                "merge-base",
                "--is-ancestor",
                &format!("{checkpoint}^1"),
                &release,
            ])
            .is_err()
        {
            return Err(fail("Fetch release after merging the UAT request. Its files must match the reviewed hotfix."));
        }
        Ok(release)
    }

    pub fn preview_hotfix(&self, membership: &str, request: &Request) -> Result<Review> {
        snapshot(&self.git, &self.path(membership)?, request)
    }
    pub fn prepare_hotfix(
        &self,
        membership: &str,
        request: &Request,
        expected: &Review,
    ) -> Result<OperationReceipt> {
        self.execute_write(membership, "prepare_hotfix", |git, root| {
            apply(git, root, request, expected)
        })
    }
}
