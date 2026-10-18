#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${1:?platform}" == windows ]]; then
  bin="$root/tools/ffmpeg/bin"
else
  bin="$root/tools/linux/ffmpeg/bin"
fi
work="$(mktemp -d)"
trap 'rm -f "$work/sample.mp4" "$work/sample.mov" "$work/sample-hevc.mp4"; rmdir "$work"' EXIT
"$bin/ffmpeg" -v error -f lavfi -i testsrc2=size=320x180:rate=30 -f lavfi -i sine=frequency=440:sample_rate=48000 \
  -t 1 -c:v libx264 -pix_fmt yuv420p -c:a aac -movflags +faststart "$work/sample.mp4"
"$bin/ffprobe" -v error -show_entries stream=codec_name,width,height,r_frame_rate -of json "$work/sample.mp4"
"$bin/ffmpeg" -v error -i "$work/sample.mp4" -f null -
"$bin/ffmpeg" -v error -i "$work/sample.mp4" -c:v prores_ks -profile:v 2 -pix_fmt yuv422p10le -c:a pcm_s16le "$work/sample.mov"
"$bin/ffmpeg" -v error -i "$work/sample.mp4" -c:v libx265 -preset ultrafast -c:a aac "$work/sample-hevc.mp4"
