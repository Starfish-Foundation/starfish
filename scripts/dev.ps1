# Run the backend and the Vite UI in two PowerShell windows.
$root = Split-Path -Parent $PSScriptRoot

Start-Process powershell -ArgumentList @(
    '-NoExit',
    '-Command',
    "Set-Location '$root'; cargo run -p bfish-backend"
)

Start-Sleep -Seconds 3

Start-Process powershell -ArgumentList @(
    '-NoExit',
    '-Command',
    "Set-Location '$root\app'; npm.cmd run dev"
)

Write-Host 'Backend: http://127.0.0.1:8765'
Write-Host 'UI:      http://localhost:5173'
Write-Host 'Studio:  install the connector with scripts\install-plugin.ps1'
