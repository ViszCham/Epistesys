$ErrorActionPreference = 'Stop'
$raw = [Console]::In.ReadToEnd()
$binary = Join-Path $env:PLUGIN_ROOT 'scripts\lc631\bin\windows-x86_64\lc631.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    [Console]::Error.WriteLine("Epistesys packaged binary is missing: $binary")
    exit 2
}
$raw | & $binary lc631-host-stop-hook
exit $LASTEXITCODE
