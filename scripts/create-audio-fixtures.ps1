$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$ffmpeg = if ($env:MERAKI_FFMPEG_DIR) { Join-Path $env:MERAKI_FFMPEG_DIR 'ffmpeg.exe' } else { Join-Path $projectRoot 'tools/ffmpeg/bin/ffmpeg.exe' }
$fixtures = Join-Path $projectRoot 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtures | Out-Null
& $ffmpeg -v error -y -f lavfi -i 'testsrc2=size=640x360:rate=30:duration=20' -f lavfi -i 'aevalsrc=0.08*sin(2*PI*440*t)+0.12*sin(2*PI*1000*t)*lt(mod(t\,1)\,0.05):s=48000:d=20' -vf "drawbox=color=white:t=fill:enable='lt(mod(t,1),0.05)'" -ac 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -b:a 192k -shortest (Join-Path $fixtures 'audio-sync.mp4')
if ($LASTEXITCODE) { throw 'Falha ao criar audio-sync.mp4' }
& $ffmpeg -v error -y -i (Join-Path $fixtures 'audio-sync.mp4') -vn -c:a pcm_s24le (Join-Path $fixtures 'audio-track.wav')
if ($LASTEXITCODE) { throw 'Falha ao criar audio-track.wav' }
Write-Output 'Fixtures Meraki: audio-sync.mp4 (flash + pulso por segundo) e audio-track.wav'
