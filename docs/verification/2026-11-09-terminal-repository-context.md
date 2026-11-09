# Terminal repository context

When the Terminal dock is visible, changing the selected repository now opens a
session for that membership or shows its existing session. Hidden sessions remain
alive; commands are never redirected by injecting `cd` into their input. The
existing four-session limit remains enforced. Reused sessions retain intentional
navigation performed by the user.

For new zsh shells, the repository root is restored after startup files, using a
quoted environment value and the shell's builtin cd. Startup files may otherwise
override the native PTY working directory. Existing user startup files remain
unchanged.

Checks run on macOS:

- Native Terminal suite: 5 tests passed. The login-shell regression now includes
  startup files that cd to `/` and verifies the resulting repository directory.
- TypeScript check passed.
- Existing Terminal browser workflow: 1 passed.
- New repository-switch browser workflow: 1 passed; verifies automatic session
  selection and reuse when returning to the first repository. Workspace/Git are
  real disposable fixtures; PTY transport is emulated in browser coverage.
- Diff whitespace check passed.

These checks do not claim Windows/Linux interactive GUI verification.
