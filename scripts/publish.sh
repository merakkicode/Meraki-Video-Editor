#!/usr/bin/env bash
set -euo pipefail
: "${MERAKI_COMMIT_EMAIL:?Set MERAKI_COMMIT_EMAIL to the team commit address}"
cd "$(dirname "${BASH_SOURCE[0]}")/.."
if [ -e .git ]; then printf '%s\n' 'Repository already initialized; review manually.' >&2; exit 1; fi
git init
git branch -M main
git config user.name 'Meraki Core Team'
git config user.email "$MERAKI_COMMIT_EMAIL"
git remote add origin https://github.com/merakkicode/meraki.git
git add .
git diff --cached --check
git commit -m 'feat(core): initial release of Meraki Video Editor v1.0.0 (Windows & Linux)'
git tag -a v1.0.0-rc1 -m 'Meraki Video Editor 1.0.0 Release Candidate'
if [ "${1:-}" = '--push' ]; then
  git push -u origin main --tags
else
  printf '%s\n' 'Local repository ready. Review git show --stat and run: git push -u origin main --tags'
fi
