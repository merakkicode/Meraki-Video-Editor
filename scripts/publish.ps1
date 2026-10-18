param([Parameter(Mandatory=$true)][string]$Email)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
function Git-Checked { & git @args; if ($LASTEXITCODE -ne 0) { throw "Git failed: $args" } }
git rev-parse --git-dir 2>$null | Out-Null
if ($LASTEXITCODE -eq 0) { throw 'Repository already exists. See docs/releasing.md for normal releases.' }
Git-Checked init -b main
Git-Checked config user.name 'Meraki Core Team'
Git-Checked config user.email $Email
Git-Checked remote add origin https://github.com/merakkicode/Meraki-Video-Editor.git
Git-Checked add .
Git-Checked diff --cached --check
Git-Checked commit -m 'feat(core): initial release of Meraki Video Editor v1.0.0 (Windows & Linux)'
Write-Output 'Review the commit, then push main. Tag only after platform validation; see docs/releasing.md.'
