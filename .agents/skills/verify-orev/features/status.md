# Status

`orev status` reads `orev.toml` in the current directory only. For a case it shows the name, version, and case root. For a workspace it shows the workspace root, registered case count, and names in registry order.

## Sub-features

- `status-case` shows the case name, version, and case root.
- `status-workspace` shows the workspace root, count, and case names.
- `status-empty-workspace` shows a count of 0.
- `status-missing` reports a missing `orev.toml` without searching parents.

## How to get to it (user POV)

- Run `orev status` from a case root or workspace root.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. Create `wks` with case `router-audit` as in [case-add.md](./case-add.md).

- **Workspace.** Run `$S send "$ID" status-workspace-01 'cd /tmp/orev-verify-'"$ID"'/wks && orev status'`. You see `=== Orev Workspace Status ===`, `Workspace Root:  /tmp/orev-verify-<ID>/wks` with your run id in place of `<ID>`, `Registered Cases: 1`, and `  - router-audit`.
- **Case.** Run `$S send "$ID" status-case-01 'cd cases/router-audit && orev status'`. You see `=== Orev Case Status ===`, `Case Name:       router-audit`, `Case Version:    0.1.0`, and `Case Root:       /tmp/orev-verify-<ID>/wks/cases/router-audit` with your run id in place of `<ID>`.
- **Empty workspace.** Run `$S send "$ID" status-empty-workspace-01 'cd /tmp/orev-verify-'"$ID"' && orev init --workspace empty >/dev/null && cd empty && orev status'`. You see `=== Orev Workspace Status ===` and `Registered Cases: 0`.
- **Missing config.** Run `$S send "$ID" status-missing-01 'cd /tmp/orev-verify-'"$ID"'/wks/kb && orev status'`. You see ``Error: `orev.toml` not found in current directory.`` and ``Please run `orev init` for a standalone case or `orev init --workspace` for a workspace.``.

## Gotchas

- `status` can print an error and still exit 0 (seen with a missing config). `EXIT=0` proves nothing. Assert on the text.
- `status` does not search parent directories, so running it from a subdirectory of a case reports a missing config.
- Workspace status reads only the registry. It does not check that the case directories exist.
