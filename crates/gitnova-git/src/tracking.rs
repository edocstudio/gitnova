use crate::{GitCli, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};

impl GitCli {
    /// Disposable display-cache identity only. Write previews still call the
    /// uncached branch_tracking method and independently validate native state.
    pub fn tracking_display_key(
        &self,
        path: &std::path::Path,
        fingerprint: &str,
    ) -> Result<String> {
        let discovery = self.discover(path)?;
        let mut hash = Sha256::new();
        hash.update(
            serde_json::to_vec(&NativePath::new(&discovery.git_dir))
                .map_err(|e| AppError::new("tracking", e.to_string()))?,
        );
        hash.update(fingerprint.as_bytes());
        hash.update(self.config_snapshot(path)?);
        Ok(format!("{:x}", hash.finalize()))
    }
    /// Batched display metadata only. Never used to authorize a push or pull.
    pub fn branch_tracking(&self, path: &std::path::Path) -> Result<Vec<BranchTracking>> {
        let shallow = self
            .text(path, &["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true";
        let options = self
            .optional_text(
                path,
                &[
                    "config",
                    "--null",
                    "--get-regexp",
                    r"^(push\.default|remote\..*\.push)$",
                ],
            )?
            .unwrap_or_default();
        let mut push_default = "simple";
        let mut explicit_push = std::collections::BTreeSet::new();
        for option in options.split('\0').filter(|s| !s.is_empty()) {
            let Some((key, value)) = option.split_once('\n') else {
                continue;
            };
            if key == "push.default" {
                push_default = value;
            }
            if let Some(remote) = key
                .strip_prefix("remote.")
                .and_then(|key| key.strip_suffix(".push"))
            {
                explicit_push.insert(remote);
            }
        }
        let track = if shallow {
            ""
        } else {
            "%(upstream:track,nobracket)"
        };
        let format = format!("--format=%(refname)%00%(objectname)%00%(upstream)%00{track}%00%(upstream:remotename)%00%(upstream:remoteref)%00%(push:remotename)%00%(push:remoteref)");
        let raw = self.text(
            path,
            &["for-each-ref", "--count=4097", &format, "refs/heads/"],
        )?;
        let rows = raw.lines().collect::<Vec<_>>();
        if rows.len() > 4096 {
            return Err(AppError::new(
                "tracking_budget",
                "Tracking display exceeds 4,096 local branches.",
            ));
        }
        rows.into_iter()
            .map(|row| {
                let f = row.split('\0').collect::<Vec<_>>();
                if f.len() != 8 {
                    return Err(AppError::new("tracking", "Invalid branch tracking record."));
                }
                crate::validate_oid(f[1])?;
                let branch = f[0]
                    .strip_prefix("refs/heads/")
                    .ok_or_else(|| AppError::new("tracking", "Invalid local branch ref."))?;
                let (state, ahead, behind) = if f[2].is_empty() {
                    ("noUpstream", None, None)
                } else if shallow {
                    ("shallow", None, None)
                } else if f[3] == "gone" {
                    ("gone", None, None)
                } else {
                    let (mut ahead, mut behind) = (0, 0);
                    for part in f[3].split(", ").filter(|s| !s.is_empty()) {
                        let (kind, count) = part
                            .split_once(' ')
                            .ok_or_else(|| AppError::new("tracking", "Invalid tracking count."))?;
                        let count = count
                            .parse::<usize>()
                            .map_err(|_| AppError::new("tracking", "Invalid tracking count."))?;
                        match kind {
                            "ahead" => ahead = count,
                            "behind" => behind = count,
                            _ => return Err(AppError::new("tracking", "Unknown tracking count.")),
                        }
                    }
                    ("tracked", Some(ahead), Some(behind))
                };
                let optional = |s: &str| (!s.is_empty()).then(|| s.to_owned());
                let push_branch =
                    f[7].strip_prefix("refs/heads/")
                        .map(str::to_owned)
                        .or_else(|| {
                            if !f[7].is_empty() || f[6].is_empty() || explicit_push.contains(f[6]) {
                                return None;
                            }
                            match push_default {
                                "current" => Some(branch.to_owned()),
                                "upstream" if f[6] == f[4] => {
                                    f[5].strip_prefix("refs/heads/").map(str::to_owned)
                                }
                                "simple"
                                    if f[6] == f[4]
                                        && f[5].strip_prefix("refs/heads/") == Some(branch) =>
                                {
                                    Some(branch.to_owned())
                                }
                                _ => None,
                            }
                        });
                Ok(BranchTracking {
                    branch: branch.into(),
                    oid: f[1].into(),
                    upstream: optional(f[2]),
                    remote: optional(f[4]),
                    remote_branch: f[5].strip_prefix("refs/heads/").map(str::to_owned),
                    push_remote: optional(f[6]),
                    push_branch,
                    ahead,
                    behind,
                    state: state.into(),
                })
            })
            .collect()
    }
}
