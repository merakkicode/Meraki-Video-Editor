$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$ffmpeg = if ($env:MERAKI_FFMPEG_DIR) { Join-Path $env:MERAKI_FFMPEG_DIR 'ffmpeg.exe' } else { Join-Path $root 'tools/ffmpeg/bin/ffmpeg.exe' }
if (!(Test-Path -LiteralPath $ffmpeg)) { throw 'Execute scripts/setup-media.ps1 primeiro.' }
$fixtures = Join-Path $root 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtures | Out-Null
$sample = Join-Path $fixtures 'sample.mp4'
& $ffmpeg -hide_banner -loglevel error -f lavfi -i 'testsrc2=size=640x360:rate=30000/1001:duration=12' -vf "drawtext=text='MERAKI Meraki  frame %{n}':fontsize=28:fontcolor=white:x=20:y=20:box=1:boxcolor=black@0.7" -c:v libx264 -g 30 -bf 2 -pix_fmt yuv420p -y $sample
if ($LASTEXITCODE) { throw 'Falha ao criar sample.mp4' }
& $ffmpeg -hide_banner -loglevel error -i $sample -c copy -output_ts_offset 5 -y (Join-Path $fixtures 'offset.mp4')
if ($LASTEXITCODE) { throw 'Falha ao criar offset.mp4' }
& $ffmpeg -hide_banner -loglevel error -i $sample -vf "select='if(lt(t,2),not(mod(n,2)),1)'" -fps_mode vfr -c:v libx264 -g 30 -bf 2 -y (Join-Path $fixtures 'vfr.mkv')
if ($LASTEXITCODE) { throw 'Falha ao criar vfr.mkv' }
Write-Output "Vídeos de teste gravados em $fixtures"
