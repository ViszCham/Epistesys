param(
  [Parameter(Mandatory=$true)][string]$PluginRoot,
  [switch]$TransportOnly,
  [string]$ReceiverRoot,
  [string]$ExpectedReceiverSha256,
  [string]$FixtureParent = [IO.Path]::GetTempPath()
)

# These helpers are for the local byte-transport test only. The receiver records
# two independent digests: the exact input JSON and the parsed assistant message.
function Get-F1Digest([byte[]]$Bytes) {
  $hasher = [Security.Cryptography.SHA256]::Create()
  try { return 'sha256:' + [BitConverter]::ToString($hasher.ComputeHash($Bytes)).Replace('-', '').ToLowerInvariant() }
  finally { $hasher.Dispose() }
}

function Invoke-F1Child([string]$Program, [string[]]$ChildArguments, [byte[]]$Wire, [string]$Data) {
  $utf8 = [Text.UTF8Encoding]::new($false, $true)
  $start = [Diagnostics.ProcessStartInfo]::new()
  $start.FileName = $Program
  $start.UseShellExecute = $false
  $start.CreateNoWindow = $true
  $start.WorkingDirectory = $Data
  $start.RedirectStandardInput = $true
  $start.RedirectStandardOutput = $true
  $start.RedirectStandardError = $true
  $start.StandardInputEncoding = $utf8
  foreach ($argValue in $ChildArguments) { $start.ArgumentList.Add($argValue) }
  $start.Environment['PLUGIN_ROOT'] = $ReceiverRoot
  $start.Environment['PLUGIN_DATA'] = $Data
  $start.Environment['CLAUDE_PLUGIN_DATA'] = $Data
  $start.Environment['TEMP'] = $Data
  $start.Environment['TMP'] = $Data
  $start.Environment['PSModuleAnalysisCachePath'] = Join-Path $Data 'powershell-module-cache'

  # Fixed-size backing makes oversized child output an error; no unbounded text reads.
  $stdout = [IO.MemoryStream]::new([byte[]]::new(4096))
  $stderr = [IO.MemoryStream]::new([byte[]]::new(32768))
  $stdout.SetLength(0)
  $stderr.SetLength(0)
  $child = [Diagnostics.Process]::new()
  $child.StartInfo = $start
  $started = $false
  try {
    $started = $child.Start()
    if (-not $started) { throw 'F1_TRANSPORT_START_FAILED' }
    $drains = [Threading.Tasks.Task[]]@(
      $child.StandardOutput.BaseStream.CopyToAsync($stdout),
      $child.StandardError.BaseStream.CopyToAsync($stderr)
    )
    $inputStream = $child.StandardInput.BaseStream
    try {
      if (-not $inputStream.WriteAsync($Wire, 0, $Wire.Length).Wait(3000)) { throw 'F1_TRANSPORT_INPUT_TIMEOUT' }
    } finally { $inputStream.Close() }
    if (-not $child.WaitForExit(20000)) { throw 'F1_TRANSPORT_CHILD_TIMEOUT' }
    if (-not [Threading.Tasks.Task]::WaitAll($drains, 2000)) { throw 'F1_TRANSPORT_CAPTURE_TIMEOUT' }
    if ($child.ExitCode -ne 0) { throw "F1_TRANSPORT_CHILD_EXIT=$($child.ExitCode)" }
    $reply = $utf8.GetString($stdout.ToArray()) | ConvertFrom-Json
    if ($reply.continue -ne $true) { throw 'F1_TRANSPORT_CONTINUE_MISSING' }
  } finally {
    try {
      if ($started -and -not $child.HasExited) {
        $child.Kill($true)
        if (-not $child.WaitForExit(2000)) { throw 'F1_TRANSPORT_CLEANUP_UNCONFIRMED' }
      }
    } finally { $child.Dispose(); $stdout.Dispose(); $stderr.Dispose() }
  }
}

if ($TransportOnly) {
  $ErrorActionPreference = 'Stop'
  # The harness uses pwsh's bounded process/tree APIs. The actual hook below is
  # always launched with Windows PowerShell, just as hooks/hooks.json specifies.
  if ($PSVersionTable.PSVersion.Major -lt 7 -or -not $IsWindows) { throw 'F1_TRANSPORT_HARNESS_REQUIRES_WINDOWS_PWSH7' }
  if (-not $ReceiverRoot) { $ReceiverRoot = $PluginRoot }
  $ReceiverRoot = (Resolve-Path -LiteralPath $ReceiverRoot).ProviderPath
  $receiver = Join-Path $ReceiverRoot 'scripts/lc631/bin/windows-x86_64/lc631.exe'
  $hook = (Resolve-Path -LiteralPath (Join-Path $PluginRoot 'hooks/run-stop-hook.ps1')).ProviderPath
  $windowsShell = Join-Path ([Environment]::SystemDirectory) 'WindowsPowerShell\v1.0\powershell.exe'
  $actualSha = (Get-FileHash -Algorithm SHA256 -LiteralPath $receiver).Hash.ToLowerInvariant()
  if ($ExpectedReceiverSha256 -notmatch '^[a-fA-F0-9]{64}$' -or $actualSha -ne $ExpectedReceiverSha256) { throw 'F1_TRANSPORT_RECEIVER_PIN_MISMATCH' }
  $fixture = Join-Path ((Resolve-Path -LiteralPath $FixtureParent).ProviderPath) ('beta7-f1-transport-' + [Guid]::NewGuid().ToString('N'))
  $null = New-Item -ItemType Directory -Path $fixture -ErrorAction Stop
  $direct = (New-Item -ItemType Directory -Path (Join-Path $fixture 'direct')).FullName
  $wrapped = (New-Item -ItemType Directory -Path (Join-Path $fixture 'hook')).FullName
  Write-Output "F1_TRANSPORT_FIXTURE root=$fixture receiver_sha256=$actualSha source_hook_sha256=$((Get-FileHash -LiteralPath $hook).Hash.ToLowerInvariant()) host_observation=unobserved"

  $utf8 = [Text.UTF8Encoding]::new($false, $true)
  $lf = [string][char]10
  $crlf = [string][char]13 + $lf
  $japanese = -join ([char[]]@(0x65e5, 0x672c, 0x8a9e))
  $emoji = [char]::ConvertFromUtf32(0x1f600)
  $message = ' ' + $japanese + ' ' + $emoji + $crlf + 'line two' + $lf
  $literal = ' ' + $japanese + ' ' + $emoji + '\r\nline two\n'
  $escaped = ' \u65e5\u672c\u8a9e \ud83d\ude00\r\nline two\n'
  $cases = @(
    @{ Name='ascii-no-final-eol'; Message='ascii'; JsonMessage='ascii'; Sep=''; End='' },
    @{ Name='literal-lf'; Message=$message; JsonMessage=$literal; Sep=$lf; End=$lf },
    @{ Name='literal-crlf'; Message=$message; JsonMessage=$literal; Sep=$crlf; End=$crlf },
    @{ Name='escaped-no-final-eol'; Message=$message; JsonMessage=$escaped; Sep=$crlf; End='' }
  )
  $session = 'beta7-f1-byte-fixture'
  $sessionDigest = Get-F1Digest ($utf8.GetBytes($session))
  $failures = [Collections.Generic.List[string]]::new()
  $sequence = 0
  foreach ($case in $cases) {
    $sequence++
    $turn = $case.Name
    $raw = (@(
      '{',
      ('"session_id":"{0}",' -f $session),
      ('"turn_id":"{0}",' -f $turn),
      '"cwd":".","hook_event_name":"Stop","permission_mode":"default","stop_hook_active":false,',
      ('"last_assistant_message":"{0}"' -f $case.JsonMessage),
      '}'
    ) -join $case.Sep) + $case.End
    $wire = $utf8.GetBytes($raw)
    $wantRaw = Get-F1Digest $wire
    $wantMessage = Get-F1Digest ($utf8.GetBytes($case.Message))
    $turnDigest = Get-F1Digest ($utf8.GetBytes($session + ':' + $turn))
    $observation = 'host-output-hook/observations/{0}-{1}.json' -f $sessionDigest.Substring(7,16),$turnDigest.Substring(7,16)
    foreach ($via in 'direct','hook') {
      if ($via -eq 'direct') {
        $data = $direct
        Invoke-F1Child $receiver @('lc631-host-stop-hook') $wire $data
      } else {
        $data = $wrapped
        Invoke-F1Child $windowsShell @('-NoProfile','-ExecutionPolicy','Bypass','-File',$hook) $wire $data
      }
      $ledger = Get-Item -LiteralPath (Join-Path $data 'host-output-hook/host-output-v2.jsonl')
      $file = Get-Item -LiteralPath (Join-Path $data $observation)
      if ($ledger.Length -gt 262144 -or $file.Length -gt 65536) { throw 'F1_TRANSPORT_FIXTURE_OUTPUT_LIMIT' }
      if ([IO.File]::ReadAllLines($ledger.FullName).Count -ne $sequence) { throw "F1_TRANSPORT_WRONG_APPEND_COUNT $via/$turn" }
      $seen = [IO.File]::ReadAllText($file.FullName, $utf8) | ConvertFrom-Json
      if ($seen.session_digest -cne $sessionDigest -or $seen.host_turn_digest -cne $turnDigest) { throw "F1_TRANSPORT_WRONG_TURN $via/$turn" }
      $rawOK = $seen.callback_payload_digest -ceq $wantRaw
      $messageOK = $seen.output_digest -ceq $wantMessage
      Write-Output "F1_TRANSPORT_CASE $via/$turn callback_exact=$rawOK message_exact=$messageOK"
      if (-not $rawOK -or -not $messageOK) {
        if ($via -eq 'direct') { throw "F1_TRANSPORT_DIRECT_CONTROL_FAILED $turn" }
        $failures.Add("$turn callback_exact=$rawOK message_exact=$messageOK")
      }
    }
  }
  if ($failures.Count) { throw ('F1_TRANSPORT_BYTE_MISMATCH ' + ($failures -join '; ')) }
  Write-Output 'F1_TRANSPORT_PASS four direct controls and four production-hook cases; host_observation=unobserved'
  exit 0
}
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
