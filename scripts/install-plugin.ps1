# Install or update the BFish Studio connector.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $root 'target\release\bfish-mcp.exe'

if (Test-Path $binary) {
    & $binary --install-plugin
    exit $LASTEXITCODE
}

Write-Host 'Release binary not found; building the .rbxmx from source...'
node (Join-Path $root 'plugin\build.mjs')
$plugins = Join-Path $env:LOCALAPPDATA 'Roblox\Plugins'
New-Item -ItemType Directory -Force -Path $plugins | Out-Null
Copy-Item (Join-Path $root 'plugin\dist\BFishConnector.rbxmx') $plugins -Force
Write-Host "Installed to $plugins"
Write-Host 'Restart Roblox Studio, then open BFish Connector from the Plugins tab.'
