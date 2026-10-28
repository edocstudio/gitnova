//! Native executable credential-helper entry, independent from Tauri/UI startup.
use super::*;

pub use gitnova_git::install_transport_credential_helper;

pub fn git_credential_helper(operation: &str, input: &[u8]) -> Result<Vec<u8>> {
    if !matches!(operation, "get" | "store" | "erase") {
        return Err(AppError::new(
            "git_auth",
            "Unsupported Git credential operation.",
        ));
    }
    // Do not persist tokens in other helpers or erase app accounts on Git rejection.
    if operation != "get" {
        return Ok(Vec::new());
    }
    if std::env::var_os("GITNOVA_CREDENTIAL_FALLBACK").is_some() {
        return Ok(Vec::new());
    }
    let targets = std::env::var("GITNOVA_CREDENTIAL_TARGETS")
        .ok()
        .filter(|v| v.len() <= 32768)
        .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .unwrap_or_default();
    if !gitnova_hosting::git_credentials::request_matches_targets(input, &targets)? {
        return Ok(Vec::new());
    }
    gitnova_hosting::git_credentials::parse_request(input)?;
    if let Some(response) = gitnova_hosting::git_credentials::response(input)? {
        return Ok(response);
    }
    // Preserve existing Git credentials when there is no connected app account.
    // GitCli clears inherited GIT_CONFIG_PARAMETERS/COUNT, including our process-
    // scoped helper, so this child cannot recurse into the native bridge.
    Ok(GitCli
        .read_input(
            &std::env::current_dir()?,
            &[
                std::ffi::OsStr::new("credential"),
                std::ffi::OsStr::new("fill"),
            ],
            input,
            16384,
        )
        .map(|output| output.bytes)
        .unwrap_or_default())
}
