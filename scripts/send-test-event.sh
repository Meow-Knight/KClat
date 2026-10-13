#!/usr/bin/env bash
# Simulates a Claude Code hook on macOS (or Linux): pipes hook-style JSON into klat-hook, exactly like Claude
# Code would. The PowerShell twin is scripts/send-test-event.ps1.
#
#   scripts/send-test-event.sh Stop                        # Claude finished a turn (wakes the cat if Ready is on)
#   scripts/send-test-event.sh PermissionRequest --tool Bash
#   scripts/send-test-event.sh PostToolUse --tool Bash     # you approved, the tool ran -> sleep
#   scripts/send-test-event.sh Notification --notification idle_prompt
#   scripts/send-test-event.sh Stop other-session --cwd ~/code/other-project
set -euo pipefail

hook="${1:?usage: send-test-event.sh <HookName> [sessionId] [--tool NAME] [--notification TYPE] [--cwd DIR]}"
shift
session="test-session"
if [[ $# -gt 0 && "$1" != --* ]]; then session="$1"; shift; fi
tool="" notification="" cwd="$PWD"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tool) tool="$2"; shift 2 ;;
    --notification) notification="$2"; shift 2 ;;
    --cwd) cwd="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

root="$(cd "$(dirname "$0")/.." && pwd)"
exe=""
for candidate in "$root"/target/{debug,release}/klat-hook{,.exe}; do
  if [[ -x "$candidate" ]]; then exe="$candidate"; break; fi
done
if [[ -z "$exe" ]]; then
  echo "klat-hook not found. Build it with: cargo build -p klat-hook" >&2
  exit 1
fi

json="{\"session_id\":\"$session\",\"transcript_path\":\"$HOME/.claude/projects/test/$session.jsonl\",\"cwd\":\"$cwd\",\"permission_mode\":\"default\",\"hook_event_name\":\"$hook\""
[[ -n "$tool" ]] && json+=",\"tool_name\":\"$tool\",\"tool_input\":{\"command\":\"echo test\"}"
[[ -n "$notification" ]] && json+=",\"notification_type\":\"$notification\",\"message\":\"test notification\""
json+="}"

printf '%s' "$json" | "$exe" "$hook"
echo "sent $hook ($session) through $exe"
