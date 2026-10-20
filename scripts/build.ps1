# Build BFish binaries, the Studio connector, the desktop UI, and the website.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    Write-Host '==> Rust release build'
    cargo build --release -p bfish-backend -p bfish-mcp

    Write-Host '==> Studio connector'
    node plugin/build.mjs

    Write-Host '==> Desktop UI'
    if (-not (Test-Path 'app\node_modules')) {
        npm.cmd --prefix app install --no-audit --no-fund
    }
    npm.cmd --prefix app run build

    Write-Host '==> Website'
    if (-not (Test-Path 'website\node_modules')) {
        npm.cmd --prefix website install --no-audit --no-fund
    }
    npm.cmd --prefix website run build

    Write-Host ''
    Write-Host 'Done:'
    Write-Host '  target\release\bfish-backend.exe'
    Write-Host '  target\release\bfish-mcp.exe'
    Write-Host '  plugin\dist\BFishConnector.rbxmx'
    Write-Host '  app\dist'
    Write-Host '  website\dist'
}
finally {
    Pop-Location
}
