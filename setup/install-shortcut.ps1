# Builds Curva, installs it for this Windows user and puts a shortcut on the desktop.
# Run it again to update. Usage: powershell -ExecutionPolicy Bypass -File setup\install-shortcut.ps1
$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
Push-Location $repository
try {
    cargo build --release -p curva-app
    if ($LASTEXITCODE -ne 0) { throw 'The build failed.' }
    $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $built = Join-Path $metadata.target_directory 'release\curva-app.exe'
} finally {
    Pop-Location
}

$installFolder = Join-Path $env:LOCALAPPDATA 'Programs\Curva'
$installed = Join-Path $installFolder 'curva.exe'
New-Item -ItemType Directory -Force -Path $installFolder | Out-Null

# Windows lets a running program be renamed but not replaced, so an open Curva is left undisturbed.
if (Test-Path $installed) {
    Get-ChildItem $installFolder -Filter 'curva.old*.exe' | Remove-Item -ErrorAction SilentlyContinue
    Move-Item -Force $installed (Join-Path $installFolder "curva.old$PID.exe")
}
Copy-Item $built $installed

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'Curva.lnk'))
$shortcut.TargetPath = $installed
$shortcut.WorkingDirectory = $installFolder
$shortcut.Save()
Write-Output "Installed $installed and added a Curva shortcut to the desktop."
