# Tool runs

`orev run` executes a tool and records its terminal output and execution details.
It does not require `orev.toml`.
It does not modify case configuration or state ledgers.

## Run a command

```bash
orev run -- readelf -h ./program
orev run -- java -jar analyzer.jar ./input.bin
orev run -- /opt/ghidra/support/analyzeHeadless /path/to/project project -import ./program
```

The tool runs in the current directory with inherited input and a pseudoterminal (PTY).
Orev displays output live and flushes it to `output.log`.
Standard output and standard error form one terminal stream.

Orev supports interactive input, colors, Ctrl-C, and terminal resize.
Full interactive behavior requires an attached terminal.
Redirected input and output also work, but the child still uses a PTY.
The log is a terminal transcript, not byte-identical ordinary pipe output.

The backend is the Rust `portable-pty` crate.
It uses a native Linux PTY.
No external `script` executable is required.
Orev supports Linux only.

## Arguments and runner options

```text
orev run [RUN_OPTIONS] -- <TOOL> [TOOL_ARGS]...
```

The separator ends Orev option parsing.
Orev does not pass that separator to the tool.
Put runner options before the separator.
Everything after the tool name belongs to the tool.
This includes `--tool-version`, `--no-version-probe`, `--help`, and any additional `--`.

Examples:

```bash
orev run --tool-version "vendor build 12" -- custom-analyzer --input ./program
orev run --no-version-probe -- ./my-script --help
orev run -- /path/to/tool --version
```

`--tool-version` supplies a version without a probe.
`--no-version-probe` skips automatic version detection.
These options cannot be used together.

For ordinary tool names, you can omit the separator:

```bash
orev run readelf -h ./program
```

Option parsing still stops at the tool name.
An unknown option before the tool name is an error.
Use the separator if the tool name starts with `-`.
`orev run --help` shows runner help.
`orev run -- readelf --help` runs the tool's help.

Orev passes arguments literally, including empty arguments and shell metacharacters.
It does not perform shell expansion, pipelines, or redirection.
For shell behavior, invoke a shell explicitly:

```bash
orev run --no-version-probe -- sh -c 'tool input | other-tool'
```

The recorded tool is then the shell, not each command inside it.

## Tool discovery

Orev resolves explicit paths or searches `PATH` for the exact tool name.
Names are case-sensitive.
The file must have executable permission.
Executable scripts use their shebang interpreter.
Orev does not add filename suffixes or select an interpreter.

## Record files

Each invocation creates `<tool-name>-<UTC-timestamp>/` in the current directory.
For example:

```text
readelf-20261028T011025Z/
├── output.log
└── metadata.yaml
```

Orev makes the tool name safe for a directory name.
Directory timestamps use UTC with second precision.
If a name exists, Orev adds `-1`, then `-2`, and so on.
It never overwrites existing records.

Orev creates directories with mode `0700` and files with mode `0600`.

### Terminal transcript

`output.log` contains terminal output after username-path redaction.
It includes ANSI sequences, control characters, and PTY line endings.
It has no session header or trailer.
Commands, times, and exit status belong in the metadata.

Terminal-echoed input is output and can appear in the transcript.
Avoid secrets in command arguments and review records before sharing them.
The [redaction limits](#username-path-redaction) also apply to terminal output.

### Execution metadata

`metadata.yaml` records:

- The argument array, execution directory, and resolved executable path.
- Tool version, its source, and probe diagnostics.
- UTC start and end times, duration, status, and exit code.
- The capture backend and environment snapshot.
- Termination signal and runner error, when applicable.

Orev writes metadata before execution and updates it after handled completion or failure.
Abrupt termination, such as SIGKILL or power loss, can leave unfinished metadata.
Early setup failures can also leave incomplete record files.
Edit `metadata.yaml` only after the run finishes; Orev rewrites it during execution.

Metadata has no `schema` or `argv_hex` fields.
For non-UTF-8 arguments, metadata substitutes U+FFFD and Orev prints a warning.
The tool still receives the original argument bytes.
String fields also undergo username-path redaction.

## Environment snapshot

The `environment` field contains:

| Field | Contents |
| --- | --- |
| `orev_version` | Orev version. |
| `os` | Operating system. |
| `kernel_release`, `machine` | Running kernel and architecture, when available. |
| `distribution` | Selected Linux distribution fields. |
| `variables` | Explicitly set locale variables, `TZ`, and `TERM`. |
| `system_timezone` | Internet Assigned Numbers Authority (IANA) timezone name, when available. |
| `diagnostics` | Collection failures, which also produce warnings. |

On Linux, distribution fields are `ID`, `VERSION_ID`, `PRETTY_NAME`, `BUILD_ID`, and `VARIANT_ID`, when present.
Orev reads `/etc/os-release`, or `/usr/lib/os-release` if the first file is absent.
Locale variables include `LANG`, `LANGUAGE`, `LC_ALL`, and standard `LC_*` categories.
Unset variables are omitted.
Empty values stay empty, and Orev does not flatten locale precedence.
An explicit `TZ` takes precedence over the system timezone.

The snapshot does not collect the full environment, credentials, hostname, or account identity as dedicated fields.
Arguments and tool output can still contain these values.
Orev obtains the current account name separately for [path redaction](#username-path-redaction).
Missing snapshot information does not prevent execution.
`--no-version-probe` still records the snapshot.
Snapshot collection does not execute extra commands.

The snapshot is not a complete reproduction bundle.
It does not capture input files, installed libraries, containers, or runtimes that a launcher selects internally.
For example, Java on `PATH` is not necessarily the runtime that Ghidra uses.
The snapshot does not claim that it is.

## Username-path redaction

Before writing logs or metadata, Orev looks up the effective user identifier (UID) in the account database.
It does not use an environment variable for the account name.

If Orev cannot identify the account, it fails rather than writing unredacted records.
It replaces exact path components that match that account name with the literal `<user>` placeholder.
For account `alice`:

| Input text | Recorded text |
| --- | --- |
| `/srv/alice/data` | `/srv/<user>/data` |
| `alice/output` | `<user>/output` |
| `/srv/alice-tools` | `/srv/alice-tools` |
| `/home/bob` | `/home/bob` |

The parent directory does not affect the match.
Matching is case-sensitive and uses `/` as the path separator.
Recognized boundaries include path separators, whitespace, control bytes, and common quote or punctuation delimiters.

Redaction covers metadata string values, both output streams, and terminal-echoed input.
It works in memory across read boundaries.
Orev does not write an unredacted temporary log.
Live terminal output, executed arguments, and the actual working directory remain unchanged.
Existing recordings remain unchanged.

**Privacy limit:** Path redaction is not full anonymization.
Standalone usernames, other accounts, secrets, and paths hidden by terminal editing or control sequences can remain.
The placeholder is not an executable path.
Restore local paths yourself before you replay a recorded command.

## Version detection

`run` and `doctor` use the same tool discovery and version rules.

| Tool | Detection method |
| --- | --- |
| Java, Javac | `-version` |
| radare2, r2 | `-v` |
| GNU `objdump`, `readelf`, `strings`, including cross-prefixed names | `--version` |
| GDB, including cross-prefixed names | `--version` |
| LLDB, binwalk, file, Python, QEMU | `--version` |
| Ghidra, analyzeHeadless | Read `application.version` from the installation's `Ghidra/application.properties`. |

Ghidra detection follows launcher symbolic links.
It does not start analysis.
Command probes have a two-second timeout and do not read standard input.
Their output does not enter the run transcript.
Unknown tools and failed probes produce warnings and record version `unknown`.
The requested command still runs.

See [Diagnostics](diagnostics.md) for doctor's tool list and Ghidra installation selection.

## Status and exit codes

| Status | Meaning |
| --- | --- |
| `preparing` | The runner has not launched the tool. |
| `running` | The tool is running. |
| `ok` | The tool returned exit code 0. |
| `failed` | The tool returned a nonzero code, including handled signals. |
| `error` | The runner failed, including detected recording failures. |

Orev normally returns the tool's exit code.
Linux signal termination uses `128 + signal`.
Runner errors return exit code 1.

**Analysis limit:** Process success does not prove analysis validity or completeness.
For example, Ghidra can return 0 after a per-file analysis timeout.
The `backend` field describes PTY capture with merged streams, not the analysis engine.

## Recording failures

An independent monitor checks the record directory and both record-file identities.
It checks at startup, once per second, and at completion, even if the tool is silent.
A missing path, replacement, or inspection failure produces an immediate terminal warning.
Once detected, that failure remains recorded.

The tool continues, but Orev ultimately returns exit code 1.
If the original metadata remains accessible, Orev records `status: error` and the actual tool exit code.
That tool exit code can still be 0.

Writes continue through the original open handles.
Orev does not recreate missing paths or overwrite replacements.
If the original metadata was deleted, its error record can become inaccessible.
Terminal diagnostics then remain the failure report.

This monitor is not tamper-proof logging.
It does not detect brief changes between checks, in-place edits, truncation, or changes after completion.
