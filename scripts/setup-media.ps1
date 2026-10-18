$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$runtime = Join-Path $root 'tools/ffmpeg'
New-Item -ItemType Directory -Force -Path $runtime | Out-Null
$archive = Join-Path $runtime 'download.zip'
# Gyan is linked by ffmpeg.org/download.html. Keep the downloaded upstream license files.
$url = 'https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip'
$expected = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
if (!(Test-Path -LiteralPath $archive)) { Invoke-WebRequest -Uri $url -OutFile $archive }
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) {
    throw 'O pacote difere do FFmpeg 9.0.2 validado. Confira a versão/hash no distribuidor ou use MERAKI_FFMPEG_DIR com uma instalação existente.'
}
Expand-Archive -LiteralPath $archive -DestinationPath $runtime -Force
$package = Get-ChildItem -LiteralPath $runtime -Directory | Where-Object Name -eq 'ffmpeg-9.0.2-essentials_build' | Select-Object -First 1
if (!$package) { throw 'Estrutura do pacote FFmpeg inesperada.' }
$bin = Join-Path $runtime 'bin'
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item -LiteralPath (Join-Path $package.FullName 'bin/ffmpeg.exe'), (Join-Path $package.FullName 'bin/ffprobe.exe') -Destination $bin
$licenses = Join-Path $runtime 'licenses'
New-Item -ItemType Directory -Force -Path $licenses | Out-Null
Copy-Item -LiteralPath (Join-Path $package.FullName 'LICENSE'), (Join-Path $package.FullName 'README.txt') -Destination $licenses
& (Join-Path $bin 'ffmpeg.exe') -version | Select-Object -First 1
Write-Output "Runtime portátil pronto em $bin"
