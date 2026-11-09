use std::io::{Read, Write};
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|v| v == "--gitnova-credential-helper")
    {
        let result = (|| -> gitnova_domain::Result<()> {
            if args.len() != 2 {
                return Err(gitnova_domain::AppError::new(
                    "git_auth",
                    "Invalid helper invocation.",
                ));
            }
            let mut input = Vec::new();
            std::io::stdin().take(16385).read_to_end(&mut input)?;
            if input.len() > 16384 {
                return Err(gitnova_domain::AppError::new(
                    "git_auth",
                    "Input too large.",
                ));
            }
            let response = gitnova_core::git_credentials::git_credential_helper(&args[1], &input)?;
            std::io::stdout().write_all(&response)?;
            Ok(())
        })();
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    let result = (|| -> gitnova_domain::Result<()> {
        gitnova_core::git_credentials::install_transport_credential_helper(
            &std::env::current_exe()?,
        )?;
        gitnova_core::nova_cli::run(
            &args,
            &std::env::current_dir()?,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
        )
    })();
    if let Err(error) = result {
        eprintln!("{}: {}", error.code, error.message);
        std::process::exit(1);
    }
}
