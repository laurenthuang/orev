# Add and convert cases

`orev case add <NAME>` registers a new case under `cases/<NAME>/` in the nearest project. Run from a standalone case, it first converts that case into a project, moves the original case to `cases/<original-name>/`, and then adds the new one.

## Sub-features

- `case-add-project` adds a case to a project and appends it to `[workspace].cases`.
- `case-add-from-case` adds a sibling when run from inside a project case.
- `case-convert` converts a standalone case into a project with two cases.
- `case-reject-name` rejects path-like, empty, `.`/`..`, and control-character names.
- `case-reject-duplicate` rejects a name that is already registered.

## How to get to it (user POV)

- Run `orev case add <NAME>` from a project root or any subdirectory.
- Run `orev case add <NAME>` from inside a project case.
- Run `orev case add <NAME>` from a standalone case root or any of its subdirectories (conversion).

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. The shell is at the scratch root.

- **Add to project.** Run `$S send "$ID" case-add-01 'orev init --workspace proj >/dev/null && cd proj && orev case add router-audit && cat orev.toml && ls -A cases/router-audit'`. The output includes ``Success! Case `router-audit` created``, `cases = ["router-audit"]` (one per line), and a case dir without local `kb`, `store`, or `tools`.
- **Shared roots.** Run `$S send "$ID" case-add-02 'cat cases/router-audit/orev.toml'`. You see `project_root = "../.."` and absolute `store_root`/`tools_root` under the project.
- **Sibling from a case.** Run `$S send "$ID" case-add-03 'cd cases/router-audit && orev case add fw && cd ../.. && cat orev.toml'`. The registry lists `router-audit` then `fw`.
- **Duplicate.** Run `$S send "$ID" case-dup-01 'orev case add fw'`. You see an error and `EXIT=1`, and the registry is unchanged.
- **Bad name.** Run `$S send "$ID" case-name-01 'orev case add "a/b"'`. You see ``Error: Failed to add case: invalid case name `a/b`: expected a single directory name`` and `EXIT=1`.
- **Convert.** Run `$S send "$ID" case-convert-01 'cd /tmp/orev-verify-'"$ID"' && orev init solo >/dev/null && cd solo && orev case add second && ls -A && cat orev.toml && cat cases/solo/orev.toml'`. The root now has `README.md cases kb orev.toml store tools` and `cases = ["solo", "second"]`. `cases/solo/orev.toml` keeps `name = "solo"` and gains `project_root` and `[storage]` roots.

## Gotchas

- orev prints a staging path (`.orev-case-stage-*/new`) while it builds the case. The final location is in the `Success!` line. Assert on that line, not on the staging path.
- A malformed nearest `orev.toml` stops the search. orev does not skip to a parent project.
- A concurrent `case add` holding the project lock makes the second one fail. Do not run two at once in one project.
- Conversion moves files. Run it only on scratch cases.
