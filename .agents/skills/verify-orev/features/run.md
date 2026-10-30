# Record a tool run

`orev run [OPTIONS] -- <TOOL> [ARGS]...` runs a tool in a PTY with live output. It writes `<tool>-<UTC>/output.txt` and `metadata.yaml` in the current directory, redacting the current account name in path components. It returns the tool's exit code (`128 + signal` for signals, 1 for record failures).

## Sub-features

- `run-record` creates a `0700` record dir with `0600` `output.txt` and `metadata.yaml`.
- `run-meta` records argv, cwd, executable, times, status, exit code, backend, and environment.
- `run-redact` replaces the account name in path components with `<user>` in the log and metadata, while live output stays unredacted.
- `run-exit` returns the tool's exit code and records `status: failed` for nonzero.
- `run-signal` handles Ctrl-C: the tool exits on SIGINT, orev returns 130, and metadata is completed.
- `run-suffix` adds `-1`, `-2`, and so on when the record name exists, never overwriting.
- `run-version` probes known tools, accepts `--tool-version`, skips with `--no-version-probe`, and rejects both together.
- `run-args` passes arguments literally with no shell.
- `run-no-config` works without any `orev.toml`.

## How to get to it (user POV)

- Run `orev run -- <tool> [args]` in any directory.
- Run `orev run <tool> [args]` without the separator for ordinary tool names.
- Run `orev run --tool-version "<v>" -- <tool>` or `orev run --no-version-probe -- <tool>`.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. The shell is at the scratch root, which has no `orev.toml`.

- **Record + redaction.** Run `$S send "$ID" run-redact-01 "printf '/home/$(id -un)/x $(id -un) /srv/$(id -un)-tools\n' > p.txt && orev run --no-version-probe -- cat p.txt"`. The live screen shows the real username path, and you see `orev: run records: <dir>` and `EXIT=0`.
- **Record proof.** Run `$S send "$ID" run-record-01 'd=$(ls -d cat-*/ | head -n 1); stat -c "%a %n" $d $d*; cat -v $d/output.txt; cat $d/metadata.yaml'`. You see `700` on the dir and `600` on both files. The log reads `/home/<user>/x <name> /srv/<name>-tools`, where `<name>` is the literal account name: only an exact path component is redacted. Metadata has `status: ok`, `exit_code: 0`, `argv: [cat, p.txt]`, `version_status: skipped`, and `backend: portable-pty (PTY, merged stdout/stderr)`. Then run `$S keep "$ID" <record-dir>`.
- **Known tool version.** Run `$S send "$ID" run-version-01 'orev run -- readelf -h /bin/ls && grep -A6 "^tool:" readelf-*/metadata.yaml'`. Metadata shows `executable: /usr/bin/readelf` and a `GNU readelf` version with a probe source.
- **Manual version.** Run `$S send "$ID" run-version-02 'orev run --tool-version "vendor build 12" -- true && grep -h "version" true-*/metadata.yaml'`. You see `version: vendor build 12`.
- **Conflict.** Run `$S send "$ID" run-version-03 'orev run --tool-version x --no-version-probe -- true'`. You see `error: the argument '--tool-version <TOOL_VERSION>' cannot be used with '--no-version-probe'` and `EXIT=2`, and no new record dir.
- **Exit code.** Run `$S send "$ID" run-exit-01 "orev run --no-version-probe -- sh -c 'exit 3'"`. You see `EXIT=3`, and `sh-*/metadata.yaml` has `status: failed` and `exit_code: 3`.
- **Ctrl-C.** Run `$S keys "$ID" -l 'orev run --no-version-probe -- sleep 30; echo "EXIT=$?"'`, then `$S keys "$ID" Enter`, `sleep 1.5`, `$S keys "$ID" C-c`, `$S wait "$ID" 10`, and `$S capture "$ID" run-signal-01`. You see `EXIT=130`, and `sleep-*/metadata.yaml` has `status: failed` and `exit_code: 130`.
- **Suffix.** Run `$S send "$ID" run-suffix-01 'orev run --no-version-probe -- true; orev run --no-version-probe -- true; ls -d true-*'`. When both runs land in the same second, you see `true-<ts>` and `true-<ts>-1`.
- **Literal args.** Run `$S send "$ID" run-args-01 "orev run --no-version-probe -- printf '[%s]\n' '\$HOME' '' 'a b'"`. The output is `[$HOME]`, `[]`, `[a b]` (no expansion, and the empty arg is kept).

## Gotchas

- The scratch root has no `orev.toml`. That is intended: `run` must not need one. Records land in the shell's current directory, so `cd` first if you want them elsewhere.
- Record names use second-precision UTC. Glob with `cat-*/`, and `ls` before asserting on an exact name.
- The live terminal is not redacted. Only the files are. Prove redaction from `output.txt`/`metadata.yaml`, not from the screen.
- `output.txt` is a PTY transcript with `\r\n` line endings and ANSI codes. Use `cat -v` or `grep -a`.
- Ctrl-C reaches orev through tmux only if the session has focus on that pane. Always `wait` for the prompt before the next `send`.
- A tool that is unknown to the probe table prints a version warning and records `unknown`. That is expected, not a failure.
