# Workspaces and configuration

This guide describes the current command-line interface (CLI).

## Terminology

Use the [canonical glossary](../glossary.md) for case, case root, workspace, workspace root, and `config_root`.

The CLI option `--workspace` creates a workspace.
It does not create a case.

## Initialize a standalone case

```bash
orev init ./my-case
```

The path is optional.
Without a path, Orev initializes the current directory.
A new case uses the directory name as its case name.
If the directory name is unavailable as UTF-8 text, Orev uses `binary_case`.
An existing configuration supplies the case name for command output and new guide files.
Initialization does not rename an existing case.
The output includes `Initializing analysis case...`, `Case Root: ...`, and `Success! Analysis case is ready.`.

The default layout is:

```text
my-case/
├── .gitignore
├── orev.toml
├── CASE.md
├── .orev/
├── kb/
│   ├── techniques/
│   ├── signatures/
│   └── pitfalls/
├── scripts/
├── state/
│   ├── index.yaml
│   └── investigation.yaml
├── store/
└── tools/
```

Initialization does not run analysis tools or create analysis-tool databases.
It creates an empty `scripts/` directory.
Put case-specific scripts in `scripts/<tool>/` as needed.
Orev does not generate or execute these scripts during initialization.

The two state files contain YAML lists.
Their initial content is `[]`.
Orev preserves existing ledgers and their comments.

### Preservation and failure limits

Orev validates an existing configuration before it creates other case resources.
Unknown fields cause an error.
Existing files and valid symbolic links remain unchanged.
Dangling links and conflicting path types cause errors.

For each missing file, Orev writes and synchronizes a temporary file in the destination directory.
It then publishes that file without overwriting an existing path.
Concurrent initialization uses the configuration that actually reached the destination.
It does not replace that configuration with another process's candidate.

Orev attempts to remove temporary files after handled failures.
Abrupt termination can leave temporary files.
Initialization is not a transaction for the whole case.
It does not guarantee rollback or durability after power loss.

## Initialize a workspace

```bash
orev init --workspace ./analysis-workspace
```

Without a path, Orev initializes the current directory.
The output includes `Initializing workspace...`, `Workspace Root: ...`, and `Success! Workspace skeleton is ready.`.
The layout is:

```text
analysis-workspace/
├── README.md
├── .gitignore
├── orev.toml
├── cases/
├── kb/
│   ├── techniques/
│   ├── signatures/
│   └── pitfalls/
├── store/
└── tools/
```

The workspace configuration starts with an empty case registry:

```toml
[workspace]
cases = []
```

This command does not create cases, state ledgers, analysis-tool databases, or a Git repository.
The `corpus/` directory is optional.
A repeated command preserves the registry and existing files.
`init --workspace` does not convert an existing standalone case.
Use `case add` for conversion.

## Add a case

From the workspace root, run:

```bash
orev case add router-audit
```

The required name identifies both the case and its directory under `cases/`.
Orev rejects an existing registered name or destination entry.
It does not reinitialize that destination.
There is no `case new` command.

Orev searches upward for the nearest `orev.toml`.
A malformed nearest configuration causes an error; Orev does not skip it.
A workspace registers case names in `[workspace].cases`.
Inside a workspace case, `workspace_root` identifies the workspace that receives the new sibling case.

New workspace cases use the same initialization engine as standalone cases.
They share the workspace's knowledge base.
Their configurations record the resolved storage roots.
They do not create local `kb/`, `store/`, or `tools/` directories.

Storage locations are fixed at case creation.
A later change to workspace storage settings does not rewrite existing case settings.
Reinitialization and doctor use the case's shared layout.
Ordinary `orev init <path>` still creates a standalone case.

### Case names

Names must be single directory names, not paths.
Orev rejects:

- Empty names, `.` and `..`.
- Control characters.
- `/`, `\`, and `:`.

These rules also apply to the original configured name during conversion and to registered case names.
Orev checks the requested name before it acquires the workspace lock.
It checks configured names under that lock before it creates case files.
Spaces inside valid names and valid Unicode names remain unchanged.
Names and shared-entry comparisons are case-sensitive.

## Convert a standalone case

Before conversion, stop analysis tools that write to the case.
Conversion moves case data.
It cannot rewrite arbitrary absolute paths inside analyst files.

Run `orev case add <new-name>` from a standalone case or one of its subdirectories.
Orev first converts that case root into a workspace root.
It moves the original case to `cases/<original-case-name>/`, then adds the requested case.

The original case name must be a valid directory name.
It must differ from the requested name.
An existing `cases/` entry at the standalone case root blocks conversion.

These resources stay at the workspace root:

- `kb/`, `store/`, and `tools/`.
- Optional `corpus/`.
- `.git`.

Other files move with the original case.
They include unknown notes, attachments, and the original README and ignore rules.
Orev creates new workspace README and ignore files at the workspace root.
It updates the original case configuration with the workspace association and storage roots.
Workspace ignore rules cover temporary state in nested cases and shared tool working data.

### Links, locking, and recovery

Before moving data, Orev checks paths and rejects symbolic links in that data.
The `cases/` entry must not be a symbolic link.

A shared directory link can remain at the workspace root only if its resolved target is outside the data to move.

A cooperative workspace lock serializes case additions.
If another addition holds the lock, Orev reports an error.
Retry only after the other operation finishes.

Handled failures trigger rollback of file moves.
If rollback cannot finish, the error identifies the retained recovery data.
Orev does not automatically recover from abrupt termination or power loss.
Inspect any retained lock and staging directory before you retry.

## Configuration

TOML is the configuration format.
The current implementation rejects unknown or unimplemented fields.
Design examples are not a list of supported configuration fields.

A minimal standalone case configuration is:

```toml
[case]
name = "my-case"
version = "0.1.0"
investigators = []
```

`investigators` defaults to an empty list.
The parser also accepts `analysts` as an alias.
Workspace cases use the top-level `workspace_root` field for their workspace association.
Generated cases under `cases/NAME/` save `workspace_root = "../.."`.
For example, a generated case in `/srv/analysis-workspace/cases/router-audit/` has:

```toml
workspace_root = "../.."

[case]
name = "router-audit"
version = "0.1.0"
investigators = []

[storage]
store_root = "/srv/analysis-workspace/store"
tools_root = "/srv/analysis-workspace/tools"
```

The storage values in this example are the resolved workspace defaults.
If the workspace uses external storage, new cases save those resolved roots instead.

Without `[storage]`, a standalone case uses local `store/` and `tools/` directories.
Optional `storage.store_root` and `storage.tools_root` values must identify existing absolute directories outside the case root.
Orev resolves symbolic links before it checks this boundary.
It does not create external roots, move data to them, or create links to them.

Workspace configurations accept the same storage fields.
Their external roots must be outside the workspace root.

### Migrate the workspace association manually

Before using an older workspace case, manually rename its top-level association field.
Keep the value unchanged:

```diff
-project_root = "../.."
+workspace_root = "../.."
```

The old `project_root` key is rejected.
There is no compatibility alias.
Configurations with both old and new fields, or duplicate `workspace_root` fields, are also rejected.
Orev does not automatically rewrite configuration or user data.

### Result formats

Generated structured results use YAML.
Terminal logs, binary inputs, Markdown guides, and checksum text keep their own formats.
Orev does not migrate, back up, or convert older configurations or result formats.
Existing user files do not change automatically.

Run [status and doctor](diagnostics.md) from a case root or workspace root.
See [Tool runs](tool-runs.md) for recording that works without case configuration.
