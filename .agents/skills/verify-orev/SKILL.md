---
name: verify-orev
description: Drive the real orev CLI (Rust, Linux) the way a user does, in an isolated tmux PTY with a disposable scratch directory, and keep terminal transcripts plus record files as proof. Use to prove behavior of `orev init`, `orev case add`, `orev status`, `orev doctor`, or `orev run` after a change, or to reproduce a user-reported CLI bug.
---

# Verify orev

orev is a short-lived CLI. Its user surface is the terminal: `init`, `case add`, `status`, `doctor`, and `run`. `run` needs a real PTY, Ctrl-C, and resize. There is no server. Each verification run builds the release binary once. It then drives it in a private tmux session whose shell starts in a private scratch directory under `/tmp`. Unit and integration tests (`cargo test`) are not a substitute: they never type into a real terminal.

All steps use one helper, run from the repo root:

```bash
S=.agents/skills/verify-orev/scripts/verify-orev.sh
$S            # prints usage
```

Choose a run id (`[A-Za-z0-9_-]+`), for example `ID=redact-$(date +%s)`. The id names everything the run owns:

| Thing | Location |
| --- | --- |
| tmux session | `orev-verify-$ID` |
| scratch (deleted at cleanup) | `/tmp/orev-verify-$ID/`, with `HOME=/tmp/orev-verify-$ID/home` |
| evidence (kept) | `target/verify-orev/$ID/` |

Two runs with different ids can run side by side. Never drive a session or directory that your run did not create. In particular, do not run orev in `demo_wks/`, which is the user's own workspace.

## Launch

```bash
$S build          # cargo build --release -> target/release/orev
$S start "$ID"    # creates scratch + evidence dirs and the tmux session
```

The session runs `bash --norc --noprofile` under `env -i`. Its `PATH` has `target/release` first, then `/usr/local/bin:/usr/bin:/bin`. It sets `LANG=C.UTF-8` and `TERM=xterm-256color`, and its prompt is `OREV-READY$ `. `start` returns once that prompt is on screen. That is the ready signal, and it means a command typed now runs under the freshly built `orev`. `start` refuses to reuse an existing session or scratch dir.

## Doctor

Run this first, and again whenever anything looks off:

```bash
$S doctor "$ID"
```

It is read-only. It checks that the binary exists and reports the `Cargo.toml` version. It checks that no file under `src/` or `Cargo.toml` is newer than the binary (otherwise run `build` again). It checks that the session exists and its cwd is inside this run's scratch dir, and that the scratch and evidence dirs exist. It ends with `DOCTOR OK` or `DOCTOR FAILED`. Do not drive on a failure.

(`orev doctor`, the product command, is a feature under test. See [features/doctor.md](features/doctor.md).)

## Drive

```bash
$S send "$ID" <evidence-name> '<shell command line>'
```

This types the line into the session, followed by `; echo "EXIT=$?"`. It waits for the prompt (60s by default; set `VERIFY_TIMEOUT=<secs>` to change it), saves the whole screen to `target/verify-orev/$ID/<evidence-name>.txt`, and prints it. `EXIT=` is the exit code of the last command in the line, so send one orev command per `send` when the exit code is the proof.

For interactive input, signals, or long runs, use the raw form:

```bash
$S keys "$ID" -l 'orev run -- sleep 30; echo "EXIT=$?"'   # -l = literal text
$S keys "$ID" Enter
sleep 1; $S keys "$ID" C-c          # tmux key names: C-c, C-d, Enter, Up...
$S wait "$ID" 10                    # until OREV-READY$ returns
$S capture "$ID" 04-ctrl-c          # save the screen + history
```

Use stable handles as assertions: orev's own lines (`orev: run records: <dir>`, `Success! ...`, `[✓]`/`[!]` doctor marks, `=== Orev ... Status ===` headers), the `EXIT=` line, and files on disk. Do not rely on colors or screen positions.

The feature map in [features/README.md](features/README.md) lists every user-facing feature with exact commands and the end state that proves it. Use it as the recipe. A proof that drives one entry point is incomplete when the map lists others.

## Evidence

Each `send` and `capture` writes `target/verify-orev/$ID/<name>.txt`. That file holds the command typed, everything orev printed (stdout and stderr share the PTY), and `EXIT=`. Copy files that orev wrote into evidence before cleanup:

```bash
$S keep "$ID" proj/cases/router-audit/cat-20261029T024655Z   # path relative to scratch
```

`keep` copies into `target/verify-orev/$ID/files/` and preserves modes, so `0700`/`0600` record permissions stay visible.

Proof standards:

- Drive the real user path, meaning the `orev` binary in a terminal. Do not call Rust functions or edit `orev.toml` by hand to set up a state the CLI should create.
- Capture the action and the resulting state. Pair each command transcript with a read-only second look at disk, such as `ls -la`, `stat -c '%a %n'`, `cat orev.toml`, `cat <record>/metadata.yaml`, or `cat -v <record>/output.log`.
- Check side effects as well as output. Look at created directories, registry entries in `[workspace].cases`, record directories, file modes, and the redacted log. A `Success!` line alone is not proof.
- `orev status` can print a configuration error and still exit 0. Its exit code proves nothing. Assert on its text.
- Do not use mocks. Use real tools for `run` (`cat`, `sh`, `sleep`, `readelf`). Fake Ghidra and fake tools (see [features/doctor.md](features/doctor.md)) are acceptable only to reach a branch that needs a specific install layout, and the report must say so.

## Cleanup

```bash
$S cleanup "$ID"
```

This kills only the `orev-verify-$ID` session and deletes only `/tmp/orev-verify-$ID`. It never kills by process name. It does not touch `target/verify-orev/$ID/`, which is the proof and survives cleanup. Run cleanup after every run, including failed and aborted ones. Remove old evidence yourself with `rm -rf target/verify-orev/<id>` once it is no longer needed.

## Helpers

- `scripts/verify-orev.sh` is the only helper. It is executable and needs `bash`, `tmux`, `awk`, `cargo`, and `git` (`doctor` only warns about git). Run it with no arguments for usage.
- On this machine `tail` is uutils: write `tail -n 5`, not `tail -5`.
