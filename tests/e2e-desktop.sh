#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

required=(Xvfb i3 i3-msg emacs emacsclient xterm xdpyinfo python3)
for command_name in "${required[@]}"; do
  command -v "$command_name" >/dev/null || {
    echo "error: desktop E2E requires $command_name" >&2
    exit 77
  }
done

cargo build --quiet --locked
binary="$repo_root/target/debug/emacs-i3"
runtime="$repo_root/target/e2e-desktop-$$"
mkdir -p "$runtime/emacs"
chmod 700 "$runtime" "$runtime/emacs"

xvfb_pid=""
i3_pid=""
xterm_pid=""
hung_pid=""
server_name="emacs-i3-e2e-$$"

cleanup() {
  local status=$?
  set +e
  if [[ -n "${display:-}" ]]; then
    DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
      emacsclient -s "$server_name" -e '(kill-emacs)' >/dev/null 2>&1
  fi
  for pid in "$hung_pid" "$xterm_pid" "$i3_pid" "$xvfb_pid"; do
    [[ -n "$pid" ]] && kill "$pid" >/dev/null 2>&1
  done
  if (( status == 0 )); then
    rm -rf "$runtime"
  else
    echo "desktop E2E failed; logs preserved under $runtime" >&2
  fi
  exit "$status"
}
trap cleanup EXIT INT TERM

fail() {
  echo "desktop E2E: $*" >&2
  return 1
}

wait_until() {
  local description=$1
  shift
  local attempt
  for attempt in $(seq 1 100); do
    if "$@"; then
      return 0
    fi
    sleep 0.05
  done
  fail "timed out waiting for $description"
}

for number in $(seq 91 140); do
  if [[ ! -S "/tmp/.X11-unix/X$number" ]]; then
    display=":$number"
    break
  fi
done
[[ -n "${display:-}" ]] || fail "no free X display in :91..:140"

cat >"$runtime/i3.config" <<'EOF'
font pango:monospace 8
focus_follows_mouse no
workspace_layout default
EOF

Xvfb "$display" -screen 0 1024x768x24 -nolisten tcp \
  >"$runtime/xvfb.log" 2>&1 &
xvfb_pid=$!
wait_until "Xvfb" env DISPLAY="$display" xdpyinfo >/dev/null 2>&1

DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
  i3 -c "$runtime/i3.config" >"$runtime/i3.log" 2>&1 &
i3_pid=$!

i3_socket=""
for _ in $(seq 1 100); do
  i3_socket="$(DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
    i3 --get-socketpath 2>/dev/null || true)"
  [[ -n "$i3_socket" && -S "$i3_socket" ]] && break
  kill -0 "$i3_pid" 2>/dev/null || {
    cat "$runtime/i3.log" >&2
    fail "nested i3 exited before creating its socket"
  }
  sleep 0.05
done
[[ -S "$i3_socket" ]] || fail "nested i3 socket was not created"

i3_call() {
  DISPLAY="$display" I3SOCK="$i3_socket" i3-msg "$@"
}

tree_json() {
  i3_call -t get_tree
}

node_id_for_class() {
  local class=$1
  tree_json | python3 -c '
import json, sys
wanted = sys.argv[1]
queue = [json.load(sys.stdin)]
while queue:
    node = queue.pop()
    if (node.get("window_properties") or {}).get("class") == wanted:
        print(node["id"])
        raise SystemExit(0)
    queue.extend(node.get("nodes", []))
    queue.extend(node.get("floating_nodes", []))
raise SystemExit(1)
' "$class"
}

node_id_for_title() {
  local title=$1
  tree_json | python3 -c '
import json, sys
wanted = sys.argv[1]
queue = [json.load(sys.stdin)]
while queue:
    node = queue.pop()
    props = node.get("window_properties") or {}
    if node.get("name") == wanted or props.get("title") == wanted:
        print(node["id"])
        raise SystemExit(0)
    queue.extend(node.get("nodes", []))
    queue.extend(node.get("floating_nodes", []))
raise SystemExit(1)
' "$title"
}

focused_class() {
  tree_json | python3 -c '
import json, sys
queue = [json.load(sys.stdin)]
while queue:
    node = queue.pop()
    if node.get("focused"):
        print((node.get("window_properties") or {}).get("class") or "")
        raise SystemExit(0)
    queue.extend(node.get("nodes", []))
    queue.extend(node.get("floating_nodes", []))
raise SystemExit(1)
'
}

emacs_eval() {
  DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
    emacsclient -s "$server_name" -e "$1"
}

frame_expr='(seq-find (lambda (f) (equal (frame-parameter f '\''name) "emacs-i3-e2e")) (frame-list))'
selected_buffer() {
  emacs_eval "(buffer-name (window-buffer (frame-selected-window $frame_expr)))" \
    | tr -d '"\n'
}

focus_emacs() {
  local id
  id="$(node_id_for_title emacs-i3-e2e)"
  i3_call "[con_id=$id] focus" >/dev/null
  [[ "$(focused_class)" == "Emacs" ]] || fail "could not focus primary Emacs frame"
}

focus_internal_left() {
  emacs_eval "(with-selected-frame $frame_expr (select-window (get-buffer-window \"*e2e-left*\" $frame_expr)) (buffer-name))" \
    >/dev/null
}

focus_internal_right() {
  emacs_eval "(with-selected-frame $frame_expr (select-window (get-buffer-window \"*e2e-right*\" $frame_expr)) (buffer-name))" \
    >/dev/null
}

run_bridge() {
  DISPLAY="$display" I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
    "$binary" --socket "$emacs_socket" "$@"
}

DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
  emacs -Q --daemon="$server_name" >"$runtime/emacs-daemon.log" 2>&1
emacs_socket="$runtime/emacs/$server_name"
[[ -S "$emacs_socket" ]] || fail "Emacs server socket was not created"
emacs_eval "(load \"$repo_root/elisp/emacs-i3.el\" nil t)" >/dev/null
DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
  emacsclient -s "$server_name" -c -n -F '((name . "emacs-i3-e2e"))' >/dev/null
wait_until "Emacs frame in i3" node_id_for_class Emacs >/dev/null

emacs_eval "(with-selected-frame $frame_expr
  (delete-other-windows)
  (switch-to-buffer (get-buffer-create \"*e2e-left*\"))
  (split-window-right)
  (other-window 1)
  (switch-to-buffer (get-buffer-create \"*e2e-right*\"))
  (other-window -1)
  (buffer-name))" >/dev/null

# Emacs-handled direction: i3 focus stays on the frame while the selected
# Emacs window moves right.
focus_emacs
focus_internal_left
[[ "$(selected_buffer)" == "*e2e-left*" ]] || fail "left Emacs window not selected"
run_bridge focus right
[[ "$(selected_buffer)" == "*e2e-right*" ]] || fail "Emacs did not handle focus right"
[[ "$(focused_class)" == "Emacs" ]] || fail "i3 focus escaped an Emacs-handled command"

# Add a real neighboring X client. At the Emacs edge, the same command must
# fall back to i3; from xterm the reverse direction must return to Emacs.
i3_call layout splith >/dev/null
DISPLAY="$display" I3SOCK="$i3_socket" xterm -fa monospace -fs 10 -T emacs-i3-e2e-xterm \
  >"$runtime/xterm.log" 2>&1 &
xterm_pid=$!
wait_until "xterm in i3" node_id_for_class XTerm >/dev/null
focus_emacs
focus_internal_right
run_bridge focus right
[[ "$(focused_class)" == "XTerm" ]] || fail "Emacs edge did not fall back to xterm"
run_bridge focus left
[[ "$(focused_class)" == "Emacs" ]] || fail "xterm did not route back to Emacs through i3"

# Tabbed and stacked containers require order-based prev/next fallback rather
# than spatial left/right commands.
for layout in tabbed stacking; do
  i3_call layout "$layout" >/dev/null
  focus_emacs
  focus_internal_right
  run_bridge focus right
  [[ "$(focused_class)" == "XTerm" ]] || fail "$layout fallback did not use next tab"
  run_bridge focus left
  [[ "$(focused_class)" == "Emacs" ]] || fail "$layout fallback did not use previous tab"
done
i3_call layout splith >/dev/null

# Floating Emacs windows still retain class detection.
focus_emacs
i3_call floating enable >/dev/null
diagnostic="$(DISPLAY="$display" I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
  "$binary" --socket "$emacs_socket" --diagnose --json)"
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["focused_is_emacs"] is True' "$diagnostic"
i3_call floating disable >/dev/null

# A second GUI frame must be independently recognized when i3 focuses it.
DISPLAY="$display" XDG_RUNTIME_DIR="$runtime" env -u I3SOCK \
  emacsclient -s "$server_name" -c -n -F '((name . "emacs-i3-e2e-second"))' >/dev/null
wait_until "second Emacs frame" node_id_for_title emacs-i3-e2e-second >/dev/null
second_id="$(node_id_for_title emacs-i3-e2e-second)"
i3_call "[con_id=$second_id] focus" >/dev/null
diagnostic="$(DISPLAY="$display" I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
  "$binary" --socket "$emacs_socket" --diagnose --json)"
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["focused_is_emacs"] is True' "$diagnostic"
emacs_eval '(delete-frame (seq-find (lambda (f) (equal (frame-parameter f '\''name) "emacs-i3-e2e-second")) (frame-list)))' >/dev/null
focus_emacs

# Missing DISPLAY must not prevent read-only diagnosis when the i3 socket is
# explicit. Missing XDG_RUNTIME_DIR must fail over to i3 instead of hanging.
diagnostic="$(env -u DISPLAY I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
  "$binary" --socket "$emacs_socket" --diagnose --json)"
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["i3_connected"] is True' "$diagnostic"

focus_emacs
focus_internal_right
start_ns="$(date +%s%N)"
env -u XDG_RUNTIME_DIR DISPLAY="$display" I3SOCK="$i3_socket" \
  "$binary" --timeout-ms 50 focus right >/dev/null 2>"$runtime/missing-runtime.log"
elapsed_ms=$(( ( $(date +%s%N) - start_ns ) / 1000000 ))
(( elapsed_ms < 1000 )) || fail "missing runtime fallback took ${elapsed_ms}ms"
[[ "$(focused_class)" == "XTerm" ]] || fail "missing runtime did not fall back to i3"

# A stale socket and a server that accepts but never replies are both bounded.
focus_emacs
focus_internal_right
start_ns="$(date +%s%N)"
DISPLAY="$display" I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
  "$binary" --socket "$runtime/stale.sock" --timeout-ms 50 focus right \
  >/dev/null 2>"$runtime/stale-socket.log"
elapsed_ms=$(( ( $(date +%s%N) - start_ns ) / 1000000 ))
(( elapsed_ms < 1000 )) || fail "stale socket fallback took ${elapsed_ms}ms"
[[ "$(focused_class)" == "XTerm" ]] || fail "stale socket did not fall back to i3"

hung_socket="$runtime/hung.sock"
python3 - "$hung_socket" <<'PY' &
import socket, sys, time
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(sys.argv[1])
server.listen(1)
connection, _ = server.accept()
time.sleep(3)
connection.close()
server.close()
PY
hung_pid=$!
wait_until "hung test socket" test -S "$hung_socket"
focus_emacs
focus_internal_right
start_ns="$(date +%s%N)"
DISPLAY="$display" I3SOCK="$i3_socket" XDG_RUNTIME_DIR="$runtime" \
  "$binary" --socket "$hung_socket" --timeout-ms 50 focus right \
  >/dev/null 2>"$runtime/hung-socket.log"
elapsed_ms=$(( ( $(date +%s%N) - start_ns ) / 1000000 ))
(( elapsed_ms < 1000 )) || fail "hung socket fallback took ${elapsed_ms}ms"
[[ "$(focused_class)" == "XTerm" ]] || fail "hung socket did not fall back to i3"

echo "desktop E2E: ok (bounded fallback ${elapsed_ms}ms)"
