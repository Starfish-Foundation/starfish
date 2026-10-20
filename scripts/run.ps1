# Launch the standalone BFish desktop app (release build).
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\bfish-app.exe'
if (-not (Test-Path $exe)) {
    Write-Host 'Release app not found. Building it now...'
    Push-Location $root
    try {
        npm.cmd --prefix app run build
        cargo build --release -p bfish-app --features custom-protocol
    }
    finally {
        Pop-Location
    }
}
Start-Process $exe
Write-Host 'BFish started. Open BFish Connector in Roblox Studio.'
