#Requires -Version 7.4
param()
# 日本語: 実launcherの子process試験。cargoは観測用関数でありbuildしない。
# English: Actual launcher subprocess tests; a sentinel function never builds.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Join-Path ([IO.Path]::GetTempPath()) ('epistesys-cache-launcher-' + [guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory -Path $root -ErrorAction Stop)
$launcher = Join-Path $root 'run-lc631.ps1'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../run-lc631.ps1') -Destination $launcher -ErrorAction Stop
$cases = 0

function Invoke-Fixture([string] $OptIn, [string] $Target, [string[]] $Arguments = @('lc631-doctor')) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = (Get-Process -Id $PID).Path
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.WorkingDirectory = $root
    $start.Environment['EPISTESYS_ALLOW_DEVELOPMENT_BUILD'] = $OptIn
    $start.Environment['EPISTESYS_DEVELOPMENT_TARGET'] = $Target
    $quoted = $launcher.Replace("'", "''")
    $tokens = ($Arguments | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }) -join ' '
    $command = "function global:cargo { [Console]::WriteLine('CARGO_SENTINEL:' + (`$args -join '|')); `$global:LASTEXITCODE=0 }; & '$quoted' $tokens"
    foreach ($argument in @('-NoProfile','-NonInteractive','-Command',$command)) {
        [void]$start.ArgumentList.Add($argument)
    }
    $child = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $child.StandardOutput.ReadToEndAsync()
        $stderr = $child.StandardError.ReadToEndAsync()
        if (-not $child.WaitForExit(30000)) { $child.Kill($true); throw 'launcher_timeout' }
        return [pscustomobject]@{ExitCode=$child.ExitCode;Text=$stdout.Result + $stderr.Result}
    } finally { $child.Dispose() }
}

function Remove-OwnedFixture {
    $expectedParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
    $item = Get-Item -LiteralPath $root -ErrorAction Stop
    if ($item.Parent.FullName.TrimEnd('\', '/') -ine $expectedParent -or
        $item.Name -cnotmatch '^epistesys-cache-launcher-[0-9a-f]{32}$' -or
        ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'fixture_root_identity' }
    $ancestor = $item.Parent
    while ($null -ne $ancestor) {
        if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'fixture_reparse_ancestor' }
        $ancestor = $ancestor.Parent
    }
    $files = @()
    $directories = @('lc631','lc631/bin','lc631/bin/windows-x86_64')
    $leaves = @('run-lc631.ps1','lc631/Cargo.toml','lc631/bin/windows-x86_64/lc631.exe')
    foreach ($relative in @('') + $directories) {
        $directory = if ($relative -ceq '') { $root } else { Join-Path $root $relative }
        if (-not (Test-Path -LiteralPath $directory)) { continue }
        $parent = Get-Item -LiteralPath $directory -ErrorAction Stop
        if (-not $parent.PSIsContainer -or ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'fixture_reparse' }
        foreach ($entry in @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'fixture_reparse' }
            $key = [IO.Path]::GetRelativePath($root, $entry.FullName).Replace('\', '/')
            if ($entry.PSIsContainer) {
                if ($directories -cnotcontains $key) { throw 'fixture_unknown_directory' }
            } elseif ($leaves -ccontains $key) { $files += $entry }
            else { throw 'fixture_unknown_entry' }
        }
    }
    foreach ($file in $files) {
        $links = @(& fsutil hardlink list $file.FullName)
        if ($LASTEXITCODE -ne 0 -or $links.Count -ne 1) { throw 'fixture_hardlink_unknown' }
        $handle = [IO.File]::Open($file.FullName, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
        $handle.Dispose()
    }
    $bytes = ($files | Measure-Object Length -Sum).Sum
    foreach ($file in $files) { Remove-Item -LiteralPath $file.FullName -ErrorAction Stop }
    foreach ($relative in @('lc631/bin/windows-x86_64','lc631/bin','lc631')) {
        $directory = Join-Path $root $relative
        if (Test-Path -LiteralPath $directory) {
            if (@(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop).Count -ne 0) { throw 'fixture_not_empty' }
            Remove-Item -LiteralPath $directory -ErrorAction Stop
        }
    }
    Remove-Item -LiteralPath $root -ErrorAction Stop
    return [pscustomobject]@{files=$files.Count;bytes=$bytes}
}

try {
foreach ($opt in @('', 'true', '01')) {
    $result = Invoke-Fixture $opt ''
    if ($result.ExitCode -eq 0 -or $result.Text.Contains('CARGO_SENTINEL:')) {
        throw 'Missing packaged binary invoked Cargo without explicit development opt-in'
    }
    $cases++
}
$result = Invoke-Fixture '1' ''
if ($result.ExitCode -eq 0 -or $result.Text.Contains('CARGO_SENTINEL:')) {
    throw 'Development build without an explicit shared target was accepted'
}
$cases++
$workspace = Join-Path $root 'lc631'
[void](New-Item -ItemType Directory -Path $workspace -ErrorAction Stop)
[IO.File]::WriteAllText((Join-Path $workspace 'Cargo.toml'), '# Test sentinel only; Cargo is never executed.')
foreach ($target in @('relative-target', [IO.Path]::GetPathRoot($root), $root, $workspace, $launcher)) {
    $result = Invoke-Fixture '1' $target
    if ($result.ExitCode -eq 0 -or $result.Text.Contains('CARGO_SENTINEL:')) {
        throw 'A non-dedicated development target was accepted'
    }
    $cases++
}
$target = Join-Path $root 'shared-target'
$result = Invoke-Fixture '1' $target
if ($result.ExitCode -ne 0 -or -not $result.Text.Contains('CARGO_SENTINEL:') -or
    -not $result.Text.Contains('|--offline|') -or
    -not $result.Text.Contains("--target-dir|$target") -or
    -not $result.Text.Trim().EndsWith('|lc631-doctor') -or (Test-Path -LiteralPath $target)) {
    throw "Explicit development invocation failed: exit=$($result.ExitCode); $($result.Text)"
}
$cases++
$junction = Join-Path $root 'owned-target-link'
[void](New-Item -ItemType Junction -Path $junction -Target $workspace -ErrorAction Stop)
try {
    $result = Invoke-Fixture '1' (Join-Path $junction 'shared-target')
    if ($result.ExitCode -eq 0 -or $result.Text.Contains('CARGO_SENTINEL:')) {
        throw 'A real reparse ancestor reached the development build route'
    }
    $cases++
} finally {
    $link = Get-Item -LiteralPath $junction -Force -ErrorAction Stop
    if (-not ($link.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        $link.LinkType -cne 'Junction' -or @($link.Target).Count -ne 1 -or
        [IO.Path]::GetFullPath(@($link.Target)[0]).TrimEnd('\', '/') -ine $workspace.TrimEnd('\', '/')) {
        throw 'fixture_link_identity_changed'
    }
    # Remove only this exact owned link, never traverse/delete its target.
    Remove-Item -LiteralPath $junction -ErrorAction Stop
    if (-not (Test-Path -LiteralPath (Join-Path $workspace 'Cargo.toml') -PathType Leaf)) {
        throw 'fixture_link_target_was_modified'
    }
}
$nativeDirectory = Join-Path $workspace 'bin/windows-x86_64'
[void](New-Item -ItemType Directory -Path $nativeDirectory -ErrorAction Stop)
$nativeSource = Join-Path ([Environment]::GetFolderPath('System')) 'cmd.exe'
$nativeItem = Get-Item -LiteralPath $nativeSource -ErrorAction Stop
if ($nativeItem.PSIsContainer -or ($nativeItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'native_fixture_source' }
# A bounded copy of an OS executable tests native argument forwarding. It is
# NOT an Epistesys binary, installation, package proof or host receipt.
Copy-Item -LiteralPath $nativeSource -Destination (Join-Path $nativeDirectory 'lc631.exe') -ErrorAction Stop
$result = Invoke-Fixture '1' $root @('/d','/c','echo PACKAGED_SENTINEL')
if ($result.ExitCode -ne 0 -or -not $result.Text.Contains('PACKAGED_SENTINEL') -or
    $result.Text.Contains('CARGO_SENTINEL:')) { throw 'packaged_native_dispatch_failed' }
$cases++
} finally {
    $cleanup = Remove-OwnedFixture
}
[pscustomobject]@{schema='epistesys-cache-launcher-test.v1';cases=$cases;result='passed';actual_cargo_build=$false;cleanup=$cleanup} | ConvertTo-Json -Compress
