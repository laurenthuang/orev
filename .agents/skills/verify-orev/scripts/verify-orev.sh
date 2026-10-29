#!/usr/bin/env bash
# Drive the real orev CLI in an isolated tmux PTY session and keep evidence.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
BIN="$REPO/target/release/orev"
PROMPT='OREV-READY$ '

usage() {
  cat <<'EOF'
usage: verify-orev.sh <command> [args]
  build                      cargo build --release (once per code change)
  start   <id>               new scratch dir + tmux session orev-verify-<id>
  doctor  <id>               read-only health check of binary, session, dirs
  send    <id> <name> <cmd>  type <cmd> in the session, wait for the prompt,
                             save the screen to evidence/<name>.txt, print EXIT
  keys    <id> <tmux-keys>   send raw keys (e.g. C-c, Enter) without waiting
  wait    <id> [secs]        wait until the prompt returns (default 30s)
  capture <id> <name>        save the full pane history to evidence/<name>.txt
  keep    <id> <path>        copy a scratch path into evidence/files/
  paths   <id>               print SCRATCH and EVIDENCE for the run
  cleanup <id>               kill only our session, delete only our scratch dir
EOF
}

paths_for() {
  ID="$1"
  [[ "$ID" =~ ^[A-Za-z0-9_-]+$ ]] || { echo "bad run id: $ID" >&2; exit 2; }
  SESSION="orev-verify-$ID"
  SCRATCH="/tmp/orev-verify-$ID"
  EVIDENCE="$REPO/target/verify-orev/$ID"
}

pane() {
  tmux capture-pane -p -t "$SESSION" -S - |
    awk '{ line[NR] = $0 } NF { last = NR } END { for (i = 1; i <= last; i++) print line[i] }'
}

wait_prompt() {
  local secs="${1:-30}" i
  for ((i = 0; i < secs * 10; i++)); do
    # Ready means the last non-empty line is a fresh prompt.
    # capture-pane trims the prompt's trailing space.
    if pane | grep -v '^$' | tail -n 1 | grep -qx 'OREV-READY\$ *'; then return 0; fi
    sleep 0.1
  done
  echo "timeout waiting for prompt in $SESSION" >&2
  return 1
}

cmd="${1:-}"; shift || true
case "$cmd" in
  build)
    cargo build --release --manifest-path "$REPO/Cargo.toml"
    echo "BIN=$BIN"
    ;;
  start)
    paths_for "$1"
    [[ -x "$BIN" ]] || { echo "missing $BIN; run: verify-orev.sh build" >&2; exit 1; }
    if tmux has-session -t "$SESSION" 2>/dev/null; then
      echo "session $SESSION already exists; pick a new id or clean up" >&2; exit 1
    fi
    [[ ! -e "$SCRATCH" ]] || { echo "$SCRATCH already exists" >&2; exit 1; }
    mkdir -p "$SCRATCH/home" "$EVIDENCE"
    # Clean shell: no user rc files, fixed prompt, release orev first on PATH.
    tmux new-session -d -s "$SESSION" -x 200 -y 50 -c "$SCRATCH" \
      env -i HOME="$SCRATCH/home" TERM=xterm-256color LANG=C.UTF-8 \
      PATH="$REPO/target/release:/usr/local/bin:/usr/bin:/bin" \
      PS1="$PROMPT" bash --norc --noprofile
    wait_prompt 10
    echo "SESSION=$SESSION SCRATCH=$SCRATCH EVIDENCE=$EVIDENCE"
    ;;
  doctor)
    paths_for "$1"; ok=1
    if [[ -x "$BIN" ]]; then
      echo "binary: $("$BIN" --version)  ($BIN)"
      want="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/Cargo.toml" | head -n1)"
      "$BIN" --version | grep -qF "$want" || { echo "FAIL: version != Cargo.toml $want"; ok=0; }
      newer="$(find "$REPO/src" "$REPO/Cargo.toml" -newer "$BIN" -print -quit)"
      [[ -z "$newer" ]] || { echo "FAIL: stale binary, $newer is newer; run build"; ok=0; }
    else
      echo "FAIL: missing $BIN"; ok=0
    fi
    if tmux has-session -t "$SESSION" 2>/dev/null; then
      cwd="$(tmux display-message -p -t "$SESSION" '#{pane_current_path}')"
      echo "session: $SESSION cwd=$cwd"
      [[ "$cwd" == "$SCRATCH"* ]] || { echo "FAIL: session cwd outside $SCRATCH"; ok=0; }
    else
      echo "FAIL: no session $SESSION"; ok=0
    fi
    [[ -d "$SCRATCH" ]] && echo "scratch: $SCRATCH" || { echo "FAIL: no $SCRATCH"; ok=0; }
    [[ -d "$EVIDENCE" ]] && echo "evidence: $EVIDENCE" || { echo "FAIL: no $EVIDENCE"; ok=0; }
    git -C "$REPO" --version >/dev/null 2>&1 && echo "git: ok" || echo "WARN: git missing"
    ((ok)) && echo "DOCTOR OK" || { echo "DOCTOR FAILED"; exit 1; }
    ;;
  send)
    paths_for "$1"; name="$2"; line="$3"
    tmux send-keys -t "$SESSION" C-l
    sleep 0.2
    tmux clear-history -t "$SESSION"
    tmux send-keys -t "$SESSION" -l -- "$line; echo \"EXIT=\$?\""
    tmux send-keys -t "$SESSION" Enter
    wait_prompt "${VERIFY_TIMEOUT:-60}"
    pane > "$EVIDENCE/$name.txt"
    cat "$EVIDENCE/$name.txt"
    ;;
  keys)
    paths_for "$1"; shift
    tmux send-keys -t "$SESSION" "$@"
    ;;
  wait)
    paths_for "$1"; wait_prompt "${2:-30}"
    ;;
  capture)
    paths_for "$1"
    pane > "$EVIDENCE/$2.txt"
    cat "$EVIDENCE/$2.txt"
    ;;
  keep)
    paths_for "$1"
    src="$(realpath -m "$SCRATCH/$2")"
    [[ "$src" == "$SCRATCH"/* && -e "$src" ]] || { echo "not in scratch: $2" >&2; exit 1; }
    mkdir -p "$EVIDENCE/files"
    cp -a "$src" "$EVIDENCE/files/"
    echo "kept $EVIDENCE/files/$(basename "$src")"
    ;;
  paths)
    paths_for "$1"; echo "SCRATCH=$SCRATCH"; echo "EVIDENCE=$EVIDENCE"
    ;;
  cleanup)
    paths_for "$1"
    tmux kill-session -t "$SESSION" 2>/dev/null && echo "killed $SESSION" || echo "no session $SESSION"
    if [[ -d "$SCRATCH" ]]; then chmod -R u+rwX "$SCRATCH"; rm -rf "$SCRATCH"; echo "removed $SCRATCH"; fi
    [[ -d "$EVIDENCE" ]] && echo "evidence kept: $EVIDENCE"
    ;;
  *) usage; exit 2 ;;
esac
