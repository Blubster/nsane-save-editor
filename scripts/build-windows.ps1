<#
.SYNOPSIS
Builds the Windows release of the editor from this clone.

.DESCRIPTION
Pulls the latest commits (unless -NoPull), runs `cargo build --release -p gui`
and prints the path of the executable. Works from any current folder: paths
are taken from the script's own location.

Needs Rust with the MSVC toolchain (rustup-init.exe), the Visual Studio Build
Tools with the "Desktop development with C++" workload, and git.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
#>
param(
    # Build the current checkout as it is.
    [switch]$NoPull
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# rustup adds this folder to the user's Path, but a shell started before the
# installation doesn't have it yet.
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path $cargoBin)) {
    $env:Path = "$cargoBin;$env:Path"
}

if (-not $NoPull) {
    git -C $root pull --ff-only
    if ($LASTEXITCODE -ne 0) { throw "git pull failed (exit code $LASTEXITCODE)" }
}

Push-Location $root
try {
    cargo build --release -p gui
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit code $LASTEXITCODE)" }
} finally {
    Pop-Location
}

$exe = Join-Path $root 'target\release\nsane-save-editor.exe'
if (-not (Test-Path $exe)) { throw "the build did not produce $exe" }
$size = (Get-Item $exe).Length
Write-Host ("Built {0} ({1:N0} bytes)" -f $exe, $size)
