#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
if git rev-parse --git-dir >/dev/null 2>&1; then
  echo 'Repository already exists. See docs/releasing.md for normal releases.' >&2
  exit 1
fi
: "${MERAKI_COMMIT_EMAIL:?Set MERAKI_COMMIT_EMAIL to the team commit email}"
git init -b main
git config user.name 'Meraki Core Team'
git config user.email "$MERAKI_COMMIT_EMAIL"
git remote add origin https://github.com/merakkicode/Meraki-Video-Editor.git
git add .
git diff --cached --check
git commit -m 'feat(core): initial release of Meraki Video Editor v1.0.0 (Windows & Linux)'
echo 'Review the commit, then push main. Tag only after platform validation; see docs/releasing.md.'
