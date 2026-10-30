# Initialize a case or workspace

`orev init` creates a standalone analysis case with guide files, state ledgers, and directories. `orev init --workspace` creates a workspace root with an empty case registry. Re-running either preserves existing files.

## Sub-features

- `init-case` creates a standalone case named after its directory.
- `init-case-cwd` initializes the current directory when no path is given.
- `init-workspace` creates a workspace with `cases = []`.
- `init-rerun` preserves existing files and ledgers on a second run.
- `init-reject-unknown` refuses an existing `orev.toml` with unknown fields.

## How to get to it (user POV)

- Run `orev init [PATH]` in a terminal.
- Run `orev init --workspace [PATH]` in a terminal.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. The shell is at the scratch root.

- **Standalone case.** Run `$S send "$ID" init-case-01 'orev init my-case'`. The output includes `Initializing analysis case...`, `Case Root: ...`, `Success! Analysis case is ready.`, and `EXIT=0`.
- **Layout proof.** Run `$S send "$ID" init-case-02 'cd my-case && ls -A && cat orev.toml state/index.yaml state/investigation.yaml && head -n 1 .gitignore && test ! -e .orev'`. You see `.gitignore orev.toml CASE.md kb scripts state store tools`, `name = "my-case"` under `[case]`, both ledgers are `[]`, and the ignore header is `# Orev Case Ignores`. There is no `.orev` entry, and `EXIT=0`.
- **Re-run preserves.** Run `$S send "$ID" init-rerun-01 'echo "- keep: 1" >> state/index.yaml && orev init . && cat state/index.yaml'`. The ledger still has the `- keep: 1` line and the run exits 0.
- **Unknown field.** Run `$S send "$ID" init-reject-01 'mkdir ../bad && printf "[case]\nname=\"bad\"\nversion=\"0.1.0\"\nbogus=1\n" > ../bad/orev.toml && orev init ../bad; ls -A ../bad'`. orev prints an error naming the unknown field, and `../bad` still contains only `orev.toml`.
- **Workspace.** Run `$S send "$ID" init-workspace-01 'cd /tmp/orev-verify-'"$ID"' && orev init --workspace wks && ls -A wks && cat wks/orev.toml && head -n 1 wks/README.md wks/.gitignore'`. The output includes `Initializing workspace...`, `Workspace Root: ...`, and `Success! Workspace skeleton is ready.`. You see `README.md .gitignore orev.toml cases kb store tools`, and `orev.toml` holds `[workspace]` with `cases = []`. The README header is `# Orev Workspace`, and the ignore header is `# Orev Workspace Ignores`.
- **Relative storage override.** Run `$S send "$ID" init-relative-01 'mkdir relative-case shared-store && printf "[case]\nname=\"relative-case\"\nversion=\"0.1.0\"\n[storage]\nstore_root=\"../shared-store\"\n" > relative-case/orev.toml && orev init relative-case'`. The output marks the scratch root's `shared-store` directory as `[external]`. The relative path uses `relative-case/` as its base, not the shell's current directory. No `relative-case/store/` is created.

## Gotchas

- `--workspace` creates a workspace, not a case. It never converts a standalone case. Use `case add` for that.
- The case name comes from the directory name. Re-running does not rename an existing case.
- Workspace cases inherit shared storage on reinitialization. Missing shared roots cause errors rather than creating case-local substitutes.
- Initialization does not run analysis tools or create a Git repository. Do not assert on `.git`.
