[CmdletBinding()]
param(
    [string]$RuntimeRoot = "$env:USERPROFILE\.cache\labyrinth-codex-v631",
    [string]$Python = "$env:LOCALAPPDATA\Programs\Python\Python312\python.exe"
)

$ErrorActionPreference = 'Stop'
$pluginRoot = Split-Path -Parent $PSScriptRoot
$requirements = Join-Path $PSScriptRoot 'requirements-media.lock.txt'
$venv = Join-Path $RuntimeRoot 'media-venv'
$venvPython = Join-Path $venv 'Scripts\python.exe'
if (-not (Test-Path -LiteralPath $Python -PathType Leaf)) {
    throw "Python executable not found: $Python"
}
if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    throw 'uv is required to install the isolated media runtime.'
}
if (-not (Test-Path -LiteralPath $venv -PathType Container)) {
    uv venv $venv --python $Python
    if ($LASTEXITCODE -ne 0) { throw 'uv venv failed' }
}
uv pip sync --python $venvPython $requirements
if ($LASTEXITCODE -ne 0) { throw 'uv pip sync failed' }

[ordered]@{
    schema_version = 'lc631-media-install.v1'
    plugin_root = $pluginRoot
    python = $venvPython
    backend_script = (Join-Path $PSScriptRoot 'media_backend.py')
    model_cache = (Join-Path $RuntimeRoot 'models')
    claim_boundary = 'dependency installation is not backend execution, model calibration, or media authority'
} | ConvertTo-Json -Depth 4
