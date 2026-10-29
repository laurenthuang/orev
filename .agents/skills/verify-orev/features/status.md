# Status

`orev status` reads `orev.toml` in the current directory only. For a case it shows the name, version, and root. For a project it shows the root, registered case count, and names in registry order.

## Sub-features

- `status-case` shows the case name, version, and root.
- `status-project` shows the project root, count, and case names.
- `status-empty-project` shows a count of 0.
- `status-missing` reports a missing `orev.toml` without searching parents.

## How to get to it (user POV)

- Run `orev status` from a case root or project root.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. Create `proj` with case `router-audit` as in [case-add.md](./case-add.md).

- **Project.** Run `$S send "$ID" status-project-01 'cd /tmp/orev-verify-'"$ID"'/proj && orev status'`. You see `=== Orev Multi-Case Project Workspace Status ===`, `Registered Cases: 1`, and `  - router-audit`.
- **Case.** Run `$S send "$ID" status-case-01 'cd cases/router-audit && orev status'`. You see `Case Name:       router-audit`, `Case Version:    0.1.0`, and the case root path.
- **Empty project.** Run `$S send "$ID" status-empty-01 'cd /tmp/orev-verify-'"$ID"' && orev init --workspace empty >/dev/null && cd empty && orev status'`. You see `Registered Cases: 0`.
- **Missing config.** Run `$S send "$ID" status-missing-01 'cd /tmp/orev-verify-'"$ID"'/proj/kb && orev status'`. You see ``Error: `orev.toml` not found in current directory.``.

## Gotchas

- `status` can print an error and still exit 0 (seen with a missing config). `EXIT=0` proves nothing. Assert on the text.
- `status` does not search parent directories, so running it from a subdirectory of a case reports a missing config.
- Project status reads only the registry. It does not check that the case directories exist.
