param(
    [Parameter(Mandatory = $true)][string] $Python,
    [Parameter(Mandatory = $true)][string] $Ffmpeg,
    [Parameter(Mandatory = $true)][string] $Models,
    [string] $TargetDirectory
)
$ErrorActionPreference = 'Stop'
foreach ($file in @($Python, $Ffmpeg)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw 'configured_real_media_tool_unavailable' }
}
if (-not (Test-Path -LiteralPath $Models -PathType Container)) { throw 'configured_model_root_unavailable' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('Epistesys owned speech fixture ' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $root) { throw 'owned_speech_fixture_collision' }
[void](New-Item -ItemType Directory -Path $root)
$speech = Join-Path $root 'owned-synthetic-speech.wav'
$voice = New-Object -ComObject SAPI.SpVoice
$voices = $voice.GetVoices()
$english = @(for ($index = 0; $index -lt $voices.Count; $index++) {
    $token = $voices.Item($index)
    if ($token.GetDescription() -match 'English') { $token }
})
if ($english.Count -eq 0) { throw 'local_English_synthetic_voice_unavailable' }
$voice.Voice = $english[0]
$stream = New-Object -ComObject SAPI.SpFileStream
try {
    # The random owned path does not exist. Mode3 writes only this fixture;
    # output goes to the stream, never to a user's speakers.
    $stream.Open($speech, 3, $false)
    $voice.AudioOutputStream = $stream
    [void]$voice.Speak('This is a local speech recognition test. The number is forty two.')
} finally { $stream.Close() }
if ((Get-Item -LiteralPath $speech).Length -le 44) { throw 'synthetic_speech_fixture_empty' }
$env:EPISTESYS_TEST_MEDIA_PYTHON = (Resolve-Path -LiteralPath $Python).Path
$env:EPISTESYS_TEST_MEDIA_FFMPEG = (Resolve-Path -LiteralPath $Ffmpeg).Path
$env:EPISTESYS_TEST_MEDIA_MODELS = (Resolve-Path -LiteralPath $Models).Path
$env:EPISTESYS_TEST_MEDIA_SPEECH = $speech
$arguments = @('test', '--manifest-path', (Join-Path $PSScriptRoot 'lc631/Cargo.toml'),
    '-p', 'lc631-media', '--test', 'epr_real_backend', '--locked', '--offline')
if ($TargetDirectory) { $arguments += @('--target-dir', $TargetDirectory) }
$arguments += @('--', '--ignored', '--nocapture', '--test-threads=1')
& cargo @arguments
if ($LASTEXITCODE -ne 0) { throw 'real_authenticated_media_observation_failed' }
Write-Output 'Owned synthetic speech decoded/transcribed; candidate observations only, no accuracy benchmark.'
