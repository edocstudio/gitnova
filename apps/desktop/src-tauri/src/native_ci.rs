//! Synthetic CI data only for an explicitly enabled, validated acceptance catalog.
//! This never reads credentials, contacts a provider, or changes Git state.
use gitnova_core::Application;
use gitnova_ipc::{
    AppError, HostProfile, HostingSelection, PipelineJob, PipelineRun, PipelineSnapshot, RepoStatus,
};
use std::sync::atomic::{AtomicBool, Ordering};
const ORIGIN: &str = "https://ci-fixture.example.invalid";
static RESOLVED: AtomicBool = AtomicBool::new(false);
pub fn enabled() -> bool {
    super::native_smoke()
        && std::env::var_os("GITNOVA_PERF").as_deref() == Some(std::ffi::OsStr::new("1"))
        && std::env::var_os("GITNOVA_NATIVE_CI_WORKFLOW").as_deref()
            == Some(std::ffi::OsStr::new("1"))
}
fn validate(app: &Application, id: &str, oid: &str) -> Result<RepoStatus, AppError> {
    if !enabled() || super::native_smoke_state()?.is_none() {
        return Err(AppError::new("smoke", "Isolated CI fixture required"));
    }
    let status = app.status(id)?;
    if status.head.as_deref() != Some(oid) {
        return Err(AppError::new(
            "smoke",
            "CI fixture must identify the current HEAD",
        ));
    }
    Ok(status)
}
pub fn connection(
    app: &Application,
    id: &str,
    oid: &str,
    profiles: &[HostProfile],
) -> Result<Option<HostingSelection>, AppError> {
    validate(app, id, oid)?;
    Ok(profiles
        .iter()
        .any(|p| p.provider == "gitlab" && p.origin == ORIGIN)
        .then(|| HostingSelection {
            remote: "origin".into(),
            branch: String::new(),
            remote_branch: None,
            oid: oid.into(),
            provider: Some("gitlab".into()),
            origin: Some(ORIGIN.into()),
        }))
}
pub fn snapshot(
    app: &Application,
    id: &str,
    selection: &HostingSelection,
    summary: bool,
) -> Result<PipelineSnapshot, AppError> {
    let status = validate(app, id, &selection.oid)?;
    if selection.remote != "origin"
        || selection.origin.as_deref() != Some(ORIGIN)
        || selection.provider.as_deref() != Some("gitlab")
    {
        return Err(AppError::new("smoke", "CI fixture scope does not match"));
    }
    let resolved = RESOLVED.load(Ordering::SeqCst);
    let jobs = if summary {
        vec![]
    } else {
        [("1", "Native deploy", false), ("2", "Native preview", true)]
            .into_iter()
            .map(|(id, name, optional)| PipelineJob {
                id: id.into(),
                name: name.into(),
                stage: "deploy".into(),
                status: if resolved { "success" } else { "manual" }.into(),
                allow_failure: Some(optional),
                runner: None,
                duration: None,
            })
            .collect()
    };
    Ok(PipelineSnapshot {
        oid: selection.oid.clone(),
        origin: ORIGIN.into(),
        project: "team/project".into(),
        partial: false,
        checked_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        runs: vec![PipelineRun {
            id: "46629".into(),
            display_number: None,
            name: None,
            sha: selection.oid.clone(),
            reference: status.branch.unwrap_or_default(),
            source: if summary { "gitlab_pipeline" } else { "push" }.into(),
            status: if resolved { "success" } else { "manual" }.into(),
            url: format!("{ORIGIN}/team/project/-/pipelines/46629"),
            jobs,
            jobs_partial: false,
        }],
    })
}
pub fn resolve() -> Result<(), AppError> {
    if !enabled() || super::native_smoke_state()?.is_none() {
        return Err(AppError::new("smoke", "Isolated CI fixture required"));
    }
    RESOLVED.store(true, Ordering::SeqCst);
    Ok(())
}
