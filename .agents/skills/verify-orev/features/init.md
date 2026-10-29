# Initialize a case or project

`orev init` creates a standalone analysis case with guide files, state ledgers, and directories. `orev init --workspace` creates a project root with an empty case registry. Re-running either preserves existing files.

## Sub-features

- `init-case` creates a standalone case named after its directory.
- `init-case-cwd` initializes the current directory when no path is given.
- `init-workspace` creates a project with `cases = []`.
- `init-rerun` preserves existing files and ledgers on a second run.
- `init-reject-unknown` refuses an existing `orev.toml` with unknown fields.

## How to get to it (user POV)

- Run `orev init [PATH]` in a terminal.
- Run `orev init --workspace [PATH]` in a terminal.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. The shell is at the scratch root.

- **Standalone case.** Run `$S send "$ID" init-case-01 'orev init my-case'`. The output ends with a success line and `EXIT=0`.
- **Layout proof.** Run `$S send "$ID" init-case-02 'cd my-case && ls -A && cat orev.toml state/index.yaml state/investigation.yaml'`. You see `.gitignore orev.toml CASE.md .orev kb scripts state store tools`, `name = "my-case"` under `[case]`, and both ledgers are `[]`.
- **Re-run preserves.** Run `$S send "$ID" init-rerun-01 'echo "- keep: 1" >> state/index.yaml && orev init . && cat state/index.yaml'`. The ledger still has the `- keep: 1` line and the run exits 0.
- **Unknown field.** Run `$S send "$ID" init-reject-01 'mkdir ../bad && printf "[case]\nname=\"bad\"\nversion=\"0.1.0\"\nbogus=1\n" > ../bad/orev.toml && orev init ../bad; ls -A ../bad'`. orev prints an error naming the unknown field, and `../bad` still contains only `orev.toml`.
- **Project.** Run `$S send "$ID" init-workspace-01 'cd /tmp/orev-verify-'"$ID"' && orev init --workspace proj && ls -A proj && cat proj/orev.toml'`. You see `README.md .gitignore orev.toml cases kb store tools`, and `orev.toml` holds `[workspace]` with `cases = []`.

## Gotchas

- `--workspace` creates a project, not a case. It never converts a standalone case. Use `case add` for that.
- The case name comes from the directory name. Re-running does not rename an existing case.
- Initialization does not run analysis tools or create a Git repository. Do not assert on `.git`.
