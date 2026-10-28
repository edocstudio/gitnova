# ADR 0001: Graph-first multi-repository workspaces

Accepted for the first slice.

Use persisted UUID identities for workspaces, repositories and worktrees. Discover Git common/per-worktree directories using Git rather than assuming `.git` is a directory. Share repository identity across linked worktrees, and maintain selection/status per membership. Do not infer submodule relationships from directory nesting.

Use native Git as the initial read adapter, with bounded subprocess execution and no shell strings. Keep application services and graph calculation independent from the desktop host. Introduce additional providers only for a demonstrated need.

Do not add a dashboard. Workspace and repository selection live in a single top-bar contextual menu. The graph remains the main surface. Selected-commit details use the existing contextual area. No release features in this milestone.

Consequences: a system Git installation is required. Deep pagination traverses earlier history again. This slice does not claim complete Git compatibility, a finished everyday client, preview execution, or multi-repo MVP acceptance.
