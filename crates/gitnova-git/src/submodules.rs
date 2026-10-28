//! Native operations only; no custom submodule update strategy is used.
use crate::{
    working::{run_fetch, run_offline_write},
    GitCli,
};
use gitnova_domain::*;
use std::{ffi::OsString, path::Path};
pub trait SubmoduleWriter {
    fn initialize_recorded(&self, parent: &Path, relative: &Path) -> Result<()>;
    fn restore_recorded(&self, child: &Path, oid: &str) -> Result<()>;
}
impl SubmoduleWriter for GitCli {
    fn initialize_recorded(&self, parent: &Path, relative: &Path) -> Result<()> {
        let mut args: Vec<OsString> = [
            "-c",
            "submodule.recurse=false",
            "-c",
            "fetch.recurseSubmodules=false",
            "submodule",
            "update",
            "--init",
            "--checkout",
            "--no-recommend-shallow",
            "--jobs=1",
            "--",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        args.push(relative.as_os_str().into());
        run_fetch(parent, &args)
    }
    fn restore_recorded(&self, child: &Path, oid: &str) -> Result<()> {
        crate::validate_oid(oid)?;
        let args: Vec<OsString> = [
            "switch",
            "--detach",
            "--no-guess",
            "--no-recurse-submodules",
            "--no-overwrite-ignore",
            oid,
        ]
        .iter()
        .map(OsString::from)
        .collect();
        run_offline_write(child, &args)
    }
}

impl GitCli {
    pub fn validate_submodule_source(&self, value: &str) -> Result<String> {
        crate::clone::validate_clone_source(value)
    }
    pub fn synchronize_submodule_urls(&self, root: &Path, path: &Path) -> Result<()> {
        crate::working::relative(&NativePath::new(path))?;
        crate::working::run_offline_write(
            root,
            &[
                "submodule".into(),
                "sync".into(),
                "--".into(),
                path.as_os_str().into(),
            ],
        )
    }
}
