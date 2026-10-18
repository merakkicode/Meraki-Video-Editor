param([Parameter(Mandatory=$true)][string]$Email, [switch]$Push)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
function Git-Checked { & git @args; if ($LASTEXITCODE -ne 0) { throw "Git failed: $args" } }
if (-not (Test-Path -LiteralPath '.git')) {
    Git-Checked init
    Git-Checked branch -M main
    Git-Checked remote add origin https://github.com/merakkicode/Meraki-Video-Editor.git
}
Git-Checked config user.name 'Meraki Core Team'
Git-Checked config user.email $Email
Git-Checked add .
Git-Checked diff --cached --check
Git-Checked commit -m 'feat(core): initial release of Meraki Video Editor v1.0.0 (Windows & Linux)'
Git-Checked tag -a v1.0.0-rc1 -m 'Meraki Video Editor 1.0.0 Release Candidate'
if ($Push) { Git-Checked push -u origin main --tags }
else { Write-Output 'Local repository ready. Review git show --stat and run: git push -u origin main --tags' }
