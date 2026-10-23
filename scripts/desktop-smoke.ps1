$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $project
New-Item -ItemType Directory -Force (Join-Path $project 'artifacts') | Out-Null
& node (Join-Path $PSScriptRoot 'desktop-smoke.mjs') *> (Join-Path $project 'artifacts/desktop-smoke.log')
exit $LASTEXITCODE
