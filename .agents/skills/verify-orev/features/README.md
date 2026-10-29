# orev verification map

This directory is the maintained source for verifying orev's user-facing CLI behavior. Read this index before driving orev, then use the matching feature file as the recipe. Commands below use `S=.agents/skills/verify-orev/scripts/verify-orev.sh` and a run id `$ID` (see [../SKILL.md](../SKILL.md)).

## Baseline preconditions

- `$S build` succeeded and `$S start "$ID"` printed the session, scratch, and evidence paths.
- `$S doctor "$ID"` prints `DOCTOR OK`.
- The session shell is at the scratch root `/tmp/orev-verify-$ID`, which is empty apart from `home/`.
- `git` and the real tools `cat`, `sh`, `sleep`, `true`, and `readelf` are on the session `PATH`.
- Never run orev against `demo_wks/` or any directory that this run did not create.

## Driving conventions

- Run each orev command through `$S send "$ID" <name> '<command>'` so the transcript and `EXIT=` land in evidence.
- Use `$S keys` plus `$S wait` only for interactive input, Ctrl-C, or resize.
- Each recipe starts at the scratch root unless its preconditions say otherwise. Start with `cd /tmp/orev-verify-$ID` in the same `send` line when unsure.
- Treat commands as literal. Keep quoting and the `--` separator unchanged.
- orev prints into the shared PTY, so stdout and stderr are one stream. Assert on text and on disk, not on stream.

## Proof and skip reporting

- CLI proof is the transcript file holding the command, the output, and `EXIT=`.
- Mutation proof adds a read-only second look at disk (`ls -la`, `cat orev.toml`, `stat -c '%a %n'`, `cat -v`), and `$S keep` of the record files.
- Record the feature id from the feature file in each evidence name, for example `run-record-01`.
- Report an unreachable path with the command tried and the unmet precondition. Do not report a skipped entry point as verified through another one.

## Feature entry contract

Each feature file has an H1 title and one paragraph about user-visible behavior, then exactly these H2s in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with verify-orev.sh` (starting with `Preconditions:` and labeled bullets of action, command, and observable result), and `Gotchas`. Keep implementation details out.

## Features

- [Initialize a case or project](./init.md) covers standalone cases, projects, layout, and preserving re-runs.
- [Add and convert cases](./case-add.md) covers `case add` in a project, conversion of a standalone case, and rejected names.
- [Status](./status.md) covers case and project status output and its exit-code limit.
- [Doctor](./doctor.md) covers directory, configuration, and tool diagnostics, including Ghidra discovery.
- [Record a tool run](./run.md) covers records, metadata, modes, redaction, exit codes, signals, and argument parsing.
