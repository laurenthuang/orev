# Glossary

These terms distinguish analysis investigations from their shared container.
Use them in source, tests, user guides, and developer documentation.

| Term | Meaning |
| --- | --- |
| Case | One analysis investigation. |
| Case root | The directory whose `orev.toml` contains `[case]`. It owns case-local files such as `CASE.md`, `state/`, and `scripts/`. It can be standalone or associated with a workspace. |
| Standalone case | A case with no associated workspace. It has local resource defaults. |
| Workspace | A multi-case container with an ordered case registry and shared resources. It is not another name for a standalone case. |
| Workspace root | The directory whose `orev.toml` contains `[workspace]`. It owns `cases/` and shared `kb/`. Storage uses local defaults or configured external roots. |
| Workspace case | A case associated with a workspace through the top-level `workspace_root` field. It inherits shared resources unless storage fields are overridden. Generated cases live in `cases/NAME/` and save `workspace_root = "../.."`. |
| Storage override | An explicit `storage.store_root` or `storage.tools_root` value in a case or workspace configuration. |
| Effective storage root | The runtime directory selected from an explicit override, inherited workspace configuration, or local default. |
| `config_root` | The directory that owns the `orev.toml` being read, either a case root or a workspace root. Use this name only when both kinds are supported. |
| Repository root | The top-level directory of the Orev source repository. It does not imply a case or workspace configuration. |
| Current working directory (`cwd`) | The directory a command runs from. It may be a case root, workspace root, subdirectory, or unrelated directory. |

Use "case root" or "workspace root" when the kind is known.
Use generic `root` only in helpers that support both configuration kinds.
Do not use "project" or "project root" as Orev domain terms.
A repository root and the current working directory (`cwd`) are separate concepts.
Neither implies an Orev workspace.

External analysis-tool databases are not Orev workspaces or cases.
For example, the database path and name passed to Ghidra's `analyzeHeadless` belong to Ghidra.
See [Tool runs](user/tool-runs.md#run-a-command).
