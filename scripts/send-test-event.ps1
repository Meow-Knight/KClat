<#
.SYNOPSIS
  Simulates a Claude Code hook: pipes hook-style JSON into klat-hook.exe, exactly like Claude Code would.

.EXAMPLE
  .\scripts\send-test-event.ps1 Stop                       # Claude finished a turn -> the cat wakes up
  .\scripts\send-test-event.ps1 UserPromptSubmit           # you sent a prompt -> the cat goes back to sleep
  .\scripts\send-test-event.ps1 PermissionRequest -ToolName Bash
  .\scripts\send-test-event.ps1 PostToolUse -ToolName Bash # you approved, the tool ran -> sleep
  .\scripts\send-test-event.ps1 Notification -NotificationType idle_prompt
  .\scripts\send-test-event.ps1 Stop other-session -Cwd C:\work\other-project
  .\scripts\send-test-event.ps1 SessionEnd
#>
param(
  [Parameter(Mandatory = $true, Position = 0)][string]$HookName,
  [Parameter(Position = 1)][string]$SessionId = 'test-session',
  [string]$ToolName,
  [string]$NotificationType,
  [string]$Cwd = (Get-Location).Path
)

$root = Split-Path -Parent $PSScriptRoot
$exe = @("$root\target\debug\klat-hook.exe", "$root\target\release\klat-hook.exe") |
  Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) {
  Write-Error "klat-hook.exe not found. Build it with: cargo build -p klat-hook"
  exit 1
}

$payload = [ordered]@{
  session_id      = $SessionId
  transcript_path = "$env:USERPROFILE\.claude\projects\test\$SessionId.jsonl"
  cwd             = $Cwd
  permission_mode = 'default'
  hook_event_name = $HookName
}
if ($ToolName) { $payload.tool_name = $ToolName; $payload.tool_input = @{ command = 'echo test' } }
if ($NotificationType) { $payload.notification_type = $NotificationType; $payload.message = 'test notification' }
$json = $payload | ConvertTo-Json -Compress

if (-not (Test-Path "$env:APPDATA\Klat\runtime.json")) {
  Write-Warning "Klat does not seem to be running (no runtime.json); the hook will exit silently."
}

# Windows PowerShell pipes ASCII to native programs by default; the hook expects UTF-8.
$OutputEncoding = New-Object System.Text.UTF8Encoding $false
$sw = [Diagnostics.Stopwatch]::StartNew()
$json | & $exe $HookName
"sent {0} (session '{1}'{2}{3}) -> hook exited {4} in {5} ms" -f $HookName, $SessionId,
  $(if ($ToolName) { ", tool $ToolName" }), $(if ($NotificationType) { ", $NotificationType" }),
  $LASTEXITCODE, $sw.ElapsedMilliseconds
