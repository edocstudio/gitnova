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

impl GitCli {
    pub fn add_submodule_registration(&self, root: &Path, source: &str, path: &Path) -> Result<()> {
        self.validate_submodule_source(source)?;
        crate::working::relative(&NativePath::new(path))?;
        run_fetch(
            root,
            &[
                "-c".into(),
                "submodule.recurse=false".into(),
                "-c".into(),
                "fetch.recurseSubmodules=false".into(),
                "-c".into(),
                "init.templateDir=".into(),
                "submodule".into(),
                "add".into(),
                "--".into(),
                source.into(),
                path.as_os_str().into(),
            ],
        )
    }
    pub fn remove_submodule_registration(
        &self,
        root: &Path,
        name: &str,
        path: &Path,
    ) -> Result<()> {
        crate::working::relative(&NativePath::new(path))?;
        // Keep the child's directory and Git metadata. Never use rm -r, deinit or --force.
        run_offline_write(
            root,
            &[
                "rm".into(),
                "--cached".into(),
                "--".into(),
                path.as_os_str().into(),
            ],
        )?;
        // --cached does not edit .gitmodules. Remove only the reviewed section.
        run_offline_write(
            root,
            &[
                "config".into(),
                "--file".into(),
                ".gitmodules".into(),
                "--remove-section".into(),
                format!("submodule.{name}").into(),
            ],
        )?;
        run_offline_write(root, &["add".into(), "--".into(), ".gitmodules".into()])
    }
}
