#!/usr/bin/env bash
# Crash test (macOS): a WHITE ROOM crash must not leave the Dock or menu bar
# hidden.
#
# Starts a debug build in smoke-test "hold" mode, which enters the workspace,
# checks from inside that its options hide the Dock and menu bar, and waits.
# This script then checks, from a separate process:
#
# 1. before launch: the presentation state of the frontmost app (the
#    baseline);
# 2. while the workspace is open: that LaunchServices records a changed
#    presentation mode for WHITE ROOM, which shows this check can see the
#    change at all;
# 3. after killing the app with SIGKILL (no clean-up code can run): that the
#    frontmost app's presentation state is back at the baseline within a few
#    seconds.
#
# The presentation state is `currentSystemPresentationOptions` together with
# LaunchServices' `UIPresentationOptions` for the app (from `lsappinfo`). The
# first alone is not enough: it reports a hidden Dock and menu bar in
# fullscreen as the default. `lsappinfo` is undocumented, which is acceptable
# in a test but is why the app itself never relies on it.
#
# Usage: scripts/crash-test.sh [path to the white-room binary inside the .app]
# Build first: npm run tauri -- build --debug --bundles app
set -euo pipefail

APP_BIN="${1:-src-tauri/target/debug/bundle/macos/WHITE ROOM.app/Contents/MacOS/white-room}"

HOLD_TIMEOUT=90   # seconds for the app to start and enter the workspace
RESTORE_TIMEOUT=10 # seconds for macOS to restore the Dock after the kill

log() { echo "[crash] $*"; }
fail() {
  log "FAIL: $*"
  if [[ -n "${output:-}" && -f "$output" ]]; then
    log "app output:"
    sed 's/^/  /' "$output"
  fi
  exit 1
}

# The presentation options of whichever app is active, as macOS applies them
# system-wide. Read from a separate process, not from WHITE ROOM itself.
system_options() {
  osascript -l JavaScript \
    -e 'ObjC.import("AppKit"); $.NSApplication.sharedApplication.currentSystemPresentationOptions'
}

# The presentation mode LaunchServices records for an app (pid or ASN).
recorded_options() {
  lsappinfo info -only UIPresentationOptions "$1" | sed -n 's/.*=//p'
}

# The presentation state of the frontmost app.
front_state() {
  local front
  front="$(lsappinfo front)"
  echo "system ${front:+$(system_options)} recorded $([[ -n "$front" ]] && recorded_options "$front")"
}

[[ "$(uname)" == "Darwin" ]] || fail "this test needs macOS"
[[ -x "$APP_BIN" ]] || fail "no app binary at $APP_BIN (build it first)"

output="$(mktemp -t white-room-crash-test)"
pid=""
cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then kill -9 "$pid" 2>/dev/null || true; fi
}
trap cleanup EXIT

baseline="$(front_state)"
log "baseline (before launch): $baseline"

WHITE_ROOM_SMOKE_TEST=hold "$APP_BIN" >"$output" 2>&1 &
pid=$!
log "started app (pid $pid); waiting for it to enter the workspace"

deadline=$((SECONDS + HOLD_TIMEOUT))
until grep -q '^\[smoke\] HOLDING' "$output"; do
  kill -0 "$pid" 2>/dev/null || fail "the app exited before entering the workspace"
  ((SECONDS < deadline)) || fail "the app did not enter the workspace within ${HOLD_TIMEOUT}s"
  sleep 0.2
done

inside="$(recorded_options "$pid")"
[[ -n "$inside" && "$inside" != "0" ]] ||
  fail "inside the workspace LaunchServices records presentation options '$inside' for WHITE ROOM; expected a changed mode"
log "inside the workspace: recorded options $inside for WHITE ROOM"


kill -9 "$pid"
wait "$pid" 2>/dev/null || true
log "killed the app with SIGKILL"

after=""
deadline=$((SECONDS + RESTORE_TIMEOUT))
while ((SECONDS < deadline)); do
  after="$(front_state)"
  if [[ "$after" == "$baseline" ]]; then
    log "after the crash: $after, back at the baseline"
    log "PASS"
    exit 0
  fi
  sleep 0.2
done
fail "${RESTORE_TIMEOUT}s after the crash the frontmost app has '$after', not the baseline '$baseline'"
