#!/usr/bin/env bash
# Retain source packages for distribution libraries included in the AppImage.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
appdir="$(find "$root/target/release/bundle/appimage" -maxdepth 1 -type d -name '*.AppDir' -print -quit)"
test -n "$appdir"
out="$root/tools/linux-sources"
mkdir -p "$out/packages"
packages="$out/packages.txt"
printf '%s\n' fonts-dejavu-core > "$packages"
while IFS= read -r file; do
  base="$(basename "$file")"
  # App libraries come from the build distribution. Record every matching owner.
  dpkg-query -S "*/$base" 2>/dev/null | sed -E 's/: \/.*$//' >> "$packages" || true
done < <(find "$appdir" -type f -name '*.so*')
sort -u "$packages" -o "$packages"
while IFS= read -r package; do
  dpkg-query -W -f='${source:Package}=${source:Version}\n' "$package"
done < "$packages" | sort -u > "$out/source-versions.txt"
cd "$out/packages"
while IFS= read -r spec; do
  apt-get source --download-only "$spec"
done < "$out/source-versions.txt"
cp "$root/scripts/collect-linux-sources.sh" "$out/"
cp /usr/share/doc/fonts-dejavu-core/copyright "$out/font-copyright.txt"
tar -czf "$root/release-assets/linux-library-sources.tar.gz" -C "$out" .
