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
Reinitialization validates configured storage before creating workspace resources.
It does not create unused local storage defaults.
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
They inherit the workspace's knowledge base and storage.
Their configurations record the workspace association, not copies of shared storage paths.
They do not create local `kb/`, `store/`, or `tools/` directories.

Workspace storage changes affect cases that inherit those settings.
Explicit case storage overrides take precedence and remain unchanged.
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
It keeps the original storage settings in the workspace configuration.
Their relative paths keep the same base directory.
The moved case gains the workspace association and inherits storage, just like the new case.
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

## Ingest samples into a case

```bash
orev sample ingest ./firmware.bin ./tree
orev sample ingest --case firmware ./firmware.bin ./tree
orev sample ingest -- "./capture with spaces.bin" ./another-directory
```

The command requires at least one source path.
Use `--` before a source name that starts with `-`.
Relative source paths use the current directory.
Orev copies each source into `00_intake/<source-basename>` at the case root.
Paths such as `.` and `..` use the resolved directory's basename.
It does not move the originals or merge directories.
Hidden files and empty directories are included.

Orev searches upward from the current directory for the nearest `orev.toml`.
Without `--case`, that configuration must describe a case, including a case associated with a workspace.
A workspace case uses its own intake, not the workspace root.
An invalid nearest configuration or missing configuration causes an error.
Orev validates the case's resource configuration before it creates intake files.

Use `--case <NAME>` to select a registered case from a workspace root or any of its subdirectories.
From a workspace case, the option selects from that case's associated workspace, including sibling cases.
The name must be a single case name, not a path.
Orev uses `cases/<NAME>`, validates its case configuration and workspace association, and rejects symlinked case directories.
An unregistered name causes an error, even if a directory with that name exists.
For a standalone case, the name must match its configured case name.
The option never creates or converts cases.
Without `--case`, running from a workspace root still causes an error.
Relative source paths continue to use the invocation directory, not the selected case directory.
The option does not add source paths or absolute paths to metadata.

The resulting layout is:

```text
my-case/
├── orev.toml
└── 00_intake/
    ├── stage.yaml
    ├── firmware.bin
    └── tree/
        ├── empty/
        └── readme.txt
```

### Intake metadata

`stage.yaml` is one YAML sequence for the entire intake.
Each successful invocation appends its samples in argument order.
Previous metadata values and copied artifacts remain unchanged.
Orev can rewrite YAML formatting when it appends.

Each sample has these fields:

- `kind` is `file` or `directory`.
- `stored_path` is the sample's name relative to `00_intake`.
- `byte_size` is the file size, or the sum of all descendant file sizes.
- `ingested_at` is the UTC ingestion timestamp in RFC 3339 format.
- `sha256` is the lowercase hexadecimal SHA-256 digest.
- `source_modified_at` is the source root's UTC modification timestamp.
- `entries` exists only for directories. It lists every descendant file and directory in sorted path order, including empty directories.

Each directory entry has a `kind` and a `path`.
File entries also have `byte_size` and `sha256`.
Every metadata path is relative to `00_intake`, including descendant paths such as `tree/readme.txt`.
Metadata does not record the original source path, its parent directories, or absolute paths.
File contents can still contain that information. Ingestion does not redact copied bytes.

For example, if both files contain the three bytes `abc`, the manifest has this shape:

```yaml
- kind: file
  stored_path: firmware.bin
  byte_size: 3
  ingested_at: '2026-10-30T01:00:00Z'
  sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
  source_modified_at: '2026-10-29T12:00:00Z'
- kind: directory
  stored_path: tree
  byte_size: 3
  ingested_at: '2026-10-30T01:00:00Z'
  sha256: c031d2f20b0085f549081c4d1459f778bb96fa4ff812427a51c888bf26bb0efa
  source_modified_at: '2026-10-29T12:00:00Z'
  entries:
  - kind: directory
    path: tree/empty
  - kind: file
    path: tree/readme.txt
    byte_size: 3
    sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

### Directory digest format

Plain files use SHA-256 over their copied contents.
Directory digests use SHA-256 over this exact binary stream:

1. Start with the ASCII bytes `orev-directory-sha256-v1` followed by one NUL byte.
2. Remove the sample's `stored_path` and the following `/` from each descendant metadata path.
   The remaining paths use UTF-8 bytes and `/` separators.
3. Sort those paths lexicographically by unsigned UTF-8 bytes.
   Include all descendant directories and files. Do not include the sample root itself.
4. For each directory, append ASCII `D`, its path's byte length as an unsigned 64-bit big-endian integer, and the path bytes.
5. For each file, append ASCII `F`, its path's byte length as an unsigned 64-bit big-endian integer, and the path bytes.
   Then append its copied byte size as an unsigned 64-bit big-endian integer and its 32 raw SHA-256 digest bytes.

There are no other separators, terminators, or entry counts.
An empty directory sample hashes only the versioned prefix.
Its digest is `aa6bb224f441e6de667f18b30de3e6d04f3724dc49def8ff903c522b3bc64a91`.
Empty descendant directories contribute their own directory entries.
The root basename, timestamps, permissions, source location, and argument order do not affect a directory digest.

### Input and collision rules

Ingestion accepts regular files and directories with UTF-8 paths.
It rejects symlinks, including symlinked source ancestors and directory contents.
Special files, including FIFOs and sockets, are unsupported.
The command rejects non-UTF-8 paths before publishing samples.
Copied directories use mode `0700`. Copied files and the manifest use mode `0600`.
Orev does not preserve executable permissions or other source filesystem attributes.

These conditions cause an error:

- A missing source or a source without a basename.
- Two arguments with the same basename.
- A basename of `stage.yaml` or one that starts with the reserved scratch prefix `.orev-ingest-`.
- Any existing destination entry, including an empty directory or a dangling symlink.
- A symlink or wrong path type at `00_intake` or `00_intake/stage.yaml`.
- A source inside `00_intake`, or a directory source that contains `00_intake`.
- Invalid existing metadata, including unknown fields, unsafe paths, duplicates, inconsistent sizes or digests, and mismatches with stored artifacts.

Orev rehashes existing recorded artifacts before it appends.
Deleting or changing an earlier artifact blocks later ingestion.
Missing input in a mixed batch prevents every sample in that invocation from being published.
Distinct basenames can be ingested in later invocations.

### Publication and failure limits

Cooperative ingests wait for an exclusive lock on the actual intake directory.
Orev stages copies and the next manifest in a private `.orev-ingest-*` directory inside `00_intake`.
It hashes copied bytes with bounded file-content memory, synchronizes staged files, and publishes samples without replacing existing paths.
After the samples reach their destinations, Orev atomically replaces the owned manifest.

A handled failure before manifest publication rolls back this invocation's published samples and removes its staging data.
It preserves previous manifest bytes and earlier artifacts.
Cleanup failures are reported with the affected paths.
An empty intake directory can remain after a failed first invocation.
If final synchronization or cleanup fails after manifest publication, Orev reports that ingestion committed rather than deleting recorded samples.

Keep sources quiescent during ingestion.
Orev checks source identity, size, modification times, and directory membership for obvious changes.
Those checks do not provide snapshot isolation.
The lock coordinates Orev ingests, not arbitrary filesystem writers.

Ingestion is not a crash-atomic multi-file transaction.
Abrupt termination can leave staging data or published samples that the old manifest does not list.
After a crash, inspect those paths before you retry.
Orev does not adopt, overwrite, or recover them automatically.

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
```

### Storage inheritance and overrides

Without `[storage]`, a standalone case uses local `store/` and `tools/` directories.
A workspace case resolves each storage field independently:

1. Use the case's explicit `storage.store_root` or `storage.tools_root`, if present.
2. Otherwise, use the associated workspace's setting.
3. If the workspace has no setting for that field, use its local `store/` or `tools/` default.

The associated workspace configuration must exist and pass schema validation.
Orev resolves workspace storage paths only for fields the case inherits.
Case initialization does not create missing workspace storage or fall back to case-local storage.

Relative storage paths resolve from the directory containing the configuration that declares them, not from the command's current directory.
For example, the workspace configuration can use sibling directories:

```toml
[workspace]
cases = ["router-audit"]

[storage]
store_root = "../shared-store"
tools_root = "../shared-tools"
```

In `/srv/analysis-workspace/orev.toml`, these paths identify `/srv/shared-store` and `/srv/shared-tools`.
Cases inherit these paths from the workspace without saving them again.

To override only one case's tools, add this block to its configuration:

```toml
[storage]
tools_root = "../../case-tools"
```

In `/srv/analysis-workspace/cases/router-audit/orev.toml`, this path identifies `/srv/analysis-workspace/case-tools`.
The case still inherits `store_root`.

Create configured storage directories before using them.
They must resolve outside the case root or workspace root that declares the override.
Orev resolves symbolic links before checking this boundary.
It does not create configured roots, move data to them, or create links to them.
Absolute paths remain supported for deliberately external storage.

Moving a workspace preserves its local defaults and relative case associations.
Relative overrides keep working if their directories retain the same relative layout.
Absolute overrides keep their original destinations.

### Let existing cases inherit workspace storage

Earlier versions saved absolute workspace storage paths in each case.
Those values remain explicit case overrides after upgrading.
Orev does not remove or rewrite them.

Check the workspace's current storage settings before opting in to inheritance.
Remove `store_root`, `tools_root`, or both from the case's `[storage]` table to inherit those fields.
You can remove the table if it is empty.
This changes path selection only.
It does not move storage data.

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
