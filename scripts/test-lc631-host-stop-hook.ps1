param([Parameter(Mandatory=$true)][string]$PluginRoot)
# lc631 host stop-hook fresh-host test (Windows twin of lc631-host-hook-test.sh)
$ErrorActionPreference = 'Continue'
$env:PLUGIN_ROOT = $PluginRoot
$hook = Join-Path $PluginRoot 'hooks\run-stop-hook.ps1'
$work = Join-Path ([IO.Path]::GetTempPath()) ("lc631test-" + [Guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Path $work | Out-Null
$script:pass = 0; $script:fail = 0

function Report([string]$name, [string]$expect, [int]$code, [string]$extra) {
  $ok = if ($expect -eq 'zero') { $code -eq 0 } else { $code -ne 0 }
  if ($ok) { $script:pass++; $r = 'PASS' } else { $script:fail++; $r = 'FAIL' }
  Write-Output "$r | $name | exit=$code | $extra"
}

function EventJson([string]$turn) {
  $cwd = $work -replace '\\','/'
  '{"session_id":"lc631-freshhost-test","transcript_path":null,"cwd":"' + $cwd + '","hook_event_name":"Stop","model":"claude-fable-5","turn_id":"' + $turn + '","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"fresh host hook test message"}'
}

function RunHook([string]$json) {
  $errFile = Join-Path $work 'stderr.txt'
  $out = $json | & powershell -NoProfile -ExecutionPolicy Bypass -File $hook 2>$errFile
  $code = $LASTEXITCODE
  $err = if (Test-Path $errFile) { (Get-Content $errFile -Raw -ErrorAction SilentlyContinue) } else { '' }
  [pscustomobject]@{ Code = $code; Out = ($out -join ' '); Err = $err }
}

# T1 happy path
$env:PLUGIN_DATA = Join-Path $work 'data1'
New-Item -ItemType Directory -Path $env:PLUGIN_DATA | Out-Null
$ledger = Join-Path $env:PLUGIN_DATA 'host-output-hook\host-output-v2.jsonl'
$r = RunHook (EventJson 'turn-1')
$code = $r.Code
if ($r.Out -notmatch '"continue":true') { $code = 98 }
$lines = if (Test-Path $ledger) { @([IO.File]::ReadAllLines($ledger)).Count } else { -1 }
Report 'T1_happy_path' zero $code "stdout=$($r.Out) ledger_lines=$lines"

# T2 second append
$r = RunHook (EventJson 'turn-2')
$code = $r.Code
$lines = if (Test-Path $ledger) { @([IO.File]::ReadAllLines($ledger)).Count } else { -1 }
if ($lines -lt 2) { $code = 98 }
Report 'T2_second_append' zero $code "ledger_lines=$lines"

# T3 malformed JSON
$r = RunHook 'not-json'
Report 'T3_malformed_json' nonzero $r.Code ("err=" + ($r.Err -replace "`r?`n", ' ').Substring(0, [Math]::Min(120, $r.Err.Length)))

# T4 PLUGIN_DATA unset
$saved = $env:PLUGIN_DATA
Remove-Item Env:PLUGIN_DATA -ErrorAction SilentlyContinue
Remove-Item Env:CLAUDE_PLUGIN_DATA -ErrorAction SilentlyContinue
$r = RunHook (EventJson 'turn-4')
$env:PLUGIN_DATA = $saved
Report 'T4_no_plugin_data' nonzero $r.Code ("err=" + ($r.Err -replace "`r?`n", ' ').Substring(0, [Math]::Min(120, $r.Err.Length)))

# T5 tampered ledger
if (Test-Path $ledger) {
  $text = [IO.File]::ReadAllText($ledger)
  $chars = $text.ToCharArray()
  $chars[19] = 'X'
  [IO.File]::WriteAllText($ledger, -join $chars)
  $r = RunHook (EventJson 'turn-5')
  Report 'T5_tampered_ledger' nonzero $r.Code ("err=" + ($r.Err -replace "`r?`n", ' ').Substring(0, [Math]::Min(120, $r.Err.Length)))
} else {
  Report 'T5_tampered_ledger' nonzero 0 'ledger missing, cannot tamper'
}

$exe = Join-Path $PluginRoot 'scripts\lc631\bin\windows-x86_64\lc631.exe'
$sha = if (Test-Path $exe) { (Get-FileHash -Algorithm SHA256 $exe).Hash } else { 'MISSING' }
Write-Output "platform=windows-x86_64 exe_sha256=$sha"
Write-Output "RESULT pass=$script:pass fail=$script:fail work=$work"
if ($script:fail -eq 0) { exit 0 } else { exit 1 }
