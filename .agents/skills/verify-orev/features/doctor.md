# Doctor

`orev doctor` reports the OS and CPU, validates the configuration in the current directory, and checks the expected directories. It then probes Java, `objdump`, `readelf`, `strings`, and Ghidra. Missing directories and tools are `[!]` warnings. Configuration and storage problems are errors.

## Sub-features

- `doctor-case` checks the case's kb/store/tools roots plus `scripts/`, `state/`, and `.orev/`.
- `doctor-workspace` checks `kb/`, `cases/`, `store/`, and `tools/` at a workspace root.
- `doctor-tools` reports versions and paths for Java and binutils.
- `doctor-ghidra` discovers Ghidra via `GHIDRA_INSTALL_DIR` or `PATH` and reads `Ghidra/application.properties`.
- `doctor-missing-dir` warns about a missing directory without failing.
- `doctor-bad-config` fails on an invalid configuration.

## How to get to it (user POV)

- Run `orev doctor` from a case root or workspace root.
- Set `GHIDRA_INSTALL_DIR=<dir>` to pick a Ghidra install.

## Driving it with verify-orev.sh

Preconditions:

- Baseline from the README. Create `wks` with case `router-audit` as in [case-add.md](./case-add.md).

- **Case.** Run `$S send "$ID" doctor-case-01 'cd /tmp/orev-verify-'"$ID"'/wks/cases/router-audit && orev doctor'`. You see `[✓] \`orev.toml\` configuration loaded and validated.`, `[✓]` lines for the workspace knowledge base at `wks/kb`, `wks/store`, `wks/tools`, and the case's `scripts`, `state`, and `.orev`, then `[✓]` tool lines with paths, and `Doctor check completed.` with `EXIT=0`.
- **Missing dir warns.** Run `$S send "$ID" doctor-missing-01 'rmdir scripts && orev doctor; mkdir scripts'`. A `[!]` line names `scripts`, and `EXIT` is still 0 (the trailing `mkdir` restores it; check the doctor line, not `EXIT`).
- **Bad config fails.** Run `$S send "$ID" doctor-bad-01 'cp orev.toml /tmp/orev-verify-'"$ID"'/home/ok.toml && echo "bogus = 1" >> orev.toml && orev doctor'`. You see a configuration error and `EXIT=1`. Restore it with `$S send "$ID" doctor-bad-02 'cp /tmp/orev-verify-'"$ID"'/home/ok.toml orev.toml'`.
- **Explicit Ghidra path.** Run `$S send "$ID" doctor-ghidra-01 'GHIDRA_INSTALL_DIR=/nonexistent orev doctor | grep Ghidra'`. You see ``[!] Ghidra: executable `/nonexistent/support/analyzeHeadless` not found or not executable``.
- **Fake Ghidra layout.** Run `$S send "$ID" doctor-ghidra-02 'G=/tmp/orev-verify-'"$ID"'/home/ghidra; mkdir -p $G/support $G/Ghidra && printf "#!/bin/sh\nexit 99\n" > $G/support/analyzeHeadless && chmod +x $G/support/analyzeHeadless && echo application.version=11.9 > $G/Ghidra/application.properties && GHIDRA_INSTALL_DIR=$G orev doctor | grep Ghidra'`. The Ghidra line shows version `11.9` without running the launcher (the launcher would exit 99). Report this as a fake install.

## Gotchas

- Ghidra is not installed on this machine, so `[!] Ghidra ... not found` is the normal result. The fake layout is the only way to reach the success branch.
- A completed doctor run does not mean every tool works. Read the `[!]` lines.
- `doctor` reads only the current directory. Workspace checks do not inspect registered cases.
- Tool versions differ between machines. Assert on `[✓] <tool>:` and the resolved path, not exact version text.
