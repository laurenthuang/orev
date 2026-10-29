# Add and convert cases

`orev case add <NAME>` registers a new case under `cases/<NAME>/` in the nearest workspace. Run from a standalone case, it first converts that case root into a workspace root, moves the original case to `cases/<original-name>/`, and then adds the new one.

## Sub-features

- `case-add-workspace` adds a case to a workspace and appends it to `[workspace].cases`.
- `case-add-from-case` adds a sibling when run from inside a workspace case.
- `case-convert` converts a standalone case into a workspace with two cases.
- `case-reject-name` rejects path-like, empty, `.`/`..`, and control-character names.
- `case-reject-duplicate` rejects a name that is already registered.

## How to get to it (user POV)

- Run `orev case add <NAME>` from a workspace root or any subdirectory.
- Run `orev case add <NAME>` from inside a workspace case.
- Run `orev case add <NAME>` from a standalone case root or any of its subdirectories (conversion).

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. The shell is at the scratch root.

- **Add to workspace.** Run `$S send "$ID" case-add-workspace-01 'orev init --workspace wks >/dev/null && cd wks && orev case add router-audit && cat orev.toml && ls -A cases/router-audit'`. The output includes ``Success! Case `router-audit` created``, `cases = ["router-audit"]` (one per line), and a case directory without local `kb`, `store`, or `tools`.
- **Shared roots.** Run `$S send "$ID" case-add-workspace-02 'cat cases/router-audit/orev.toml'`. You see top-level `workspace_root = "../.."` and no `[storage]` table. Unset storage fields inherit the workspace settings or defaults.
- **Sibling from a case.** Run `$S send "$ID" case-add-03 'cd cases/router-audit && orev case add fw && cd ../.. && cat orev.toml'`. The registry lists `router-audit` then `fw`.
- **Duplicate.** Run `$S send "$ID" case-dup-01 'orev case add fw'`. You see an error and `EXIT=1`, and the registry is unchanged.
- **Bad name.** Run `$S send "$ID" case-name-01 'orev case add "a/b"'`. You see ``Error: Failed to add case: invalid case name `a/b`: expected a single directory name`` and `EXIT=1`.
- **Convert.** Run `$S send "$ID" case-convert-01 'cd /tmp/orev-verify-'"$ID"' && orev init solo >/dev/null && cd solo && orev case add second && ls -A && cat orev.toml && cat cases/solo/orev.toml'`. The workspace root now has `README.md cases kb orev.toml store tools` and `cases = ["solo", "second"]`. `cases/solo/orev.toml` keeps `name = "solo"` and gains `workspace_root`, with no `[storage]` table. Configured standalone storage stays in the workspace configuration, where relative paths keep the same base.

## Gotchas

- orev prints a staging path (`.orev-case-stage-*/new`) while it builds the case. The final location is in the `Success!` line. Assert on that line, not on the staging path.
- A malformed nearest `orev.toml` stops the search. orev does not skip to an ancestor workspace.
- A concurrent `case add` holding the workspace lock makes the second one fail. Do not run two at once in one workspace.
- Older workspace cases need the [manual association-field migration](../../../../docs/user/workspaces.md#migrate-the-workspace-association-manually). Reinitialization does not rewrite them.
- Older generated storage paths remain explicit overrides. Follow [the storage inheritance steps](../../../../docs/user/workspaces.md#let-existing-cases-inherit-workspace-storage) to opt in manually.
- Conversion moves files. Run it only on scratch cases.
