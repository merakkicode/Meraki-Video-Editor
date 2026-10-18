$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (!(Test-Path -LiteralPath (Join-Path $projectRoot 'tools/ffmpeg/bin/ffmpeg.exe'))) {
    & (Join-Path $PSScriptRoot 'setup-media.ps1')
}
Push-Location (Join-Path $projectRoot 'apps/desktop')
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw 'npm ci falhou' }
    npm run bundle:windows
    if ($LASTEXITCODE -ne 0) { throw 'NSIS build failed' }
} finally { Pop-Location }
Write-Output "Installer: $projectRoot/target/release/bundle/nsis/"
