# Status

`orev status` searches the current directory and its parents for the nearest `orev.toml`. For a case it shows the name, version, and case root. For a workspace it shows the workspace root, registered case count, and names in registry order.

## Sub-features

- `status-case` shows the case name, version, and case root.
- `status-workspace` shows the workspace root, count, and case names.
- `status-empty-workspace` shows a count of 0.
- `status-nested` uses the nearest case or workspace configuration from a subdirectory.
- `status-missing` reports that no `orev.toml` exists in the current directory or its parents.

## How to get to it (user POV)

- Run `orev status` from a case root, workspace root, or one of their subdirectories.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. Create `wks` with case `router-audit` as in [case-add.md](./case-add.md).

- **Workspace.** Run `$S send "$ID" status-workspace-01 'cd /tmp/orev-verify-'"$ID"'/wks && orev status'`. You see `=== Orev Workspace Status ===`, `Workspace Root:  /tmp/orev-verify-<ID>/wks` with your run id in place of `<ID>`, `Registered Cases: 1`, and `  - router-audit`.
- **Case.** Run `$S send "$ID" status-case-01 'cd cases/router-audit && orev status'`. You see `=== Orev Case Status ===`, `Case Name:       router-audit`, `Case Version:    0.1.0`, and `Case Root:       /tmp/orev-verify-<ID>/wks/cases/router-audit` with your run id in place of `<ID>`.
- **Empty workspace.** Run `$S send "$ID" status-empty-workspace-01 'cd /tmp/orev-verify-'"$ID"' && orev init --workspace empty >/dev/null && cd empty && orev status'`. You see `=== Orev Workspace Status ===` and `Registered Cases: 0`.
- **Nested workspace.** Run `$S send "$ID" status-nested-workspace-01 'cd /tmp/orev-verify-'"$ID"'/wks/kb/techniques && orev status'`. The workspace status reports `wks` as the root, not `kb/techniques`.
- **Nested case.** Run `$S send "$ID" status-nested-case-01 'cd /tmp/orev-verify-'"$ID"'/wks/cases/router-audit/scripts && orev status'`. The case status reports `router-audit` as the root, not `scripts` or the workspace.
- **Missing config.** Run `$S send "$ID" status-missing-01 'cd /tmp/orev-verify-'"$ID"'/home && orev status'`. You see ``Error: `orev.toml` not found in current directory or its ancestors.`` and ``Please run `orev init` for a standalone case or `orev init --workspace` for a workspace.``.

## Gotchas

- `status` can print an error and still exit 0 (seen with a missing config). `EXIT=0` proves nothing. Assert on the text.
- A malformed or unreadable nearest configuration causes an error. Status does not skip it for an ancestor configuration.
- Workspace status reads only the registry. It does not check that the case directories exist.
