#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ ! -x "$project_root/tools/linux/ffmpeg/bin/ffmpeg" ]; then
  bash "$project_root/scripts/setup-media.sh"
fi
cd "$project_root/apps/desktop"
npm ci
npm run bundle:linux
