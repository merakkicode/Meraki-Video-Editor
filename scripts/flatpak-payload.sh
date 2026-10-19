#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
payload="$stage/payload"
mkdir -p "$payload/bin" "$payload/lib/meraki" "$payload/share/fonts/truetype/dejavu" "$payload/etc"
install -m755 "$project_root/target/release/meraki-desktop" "$payload/bin/meraki-desktop.real"
cp -a "$project_root/tools/linux/ffmpeg" "$payload/lib/meraki/"
# Match the native lookup path /app/lib/meraki/tools/ffmpeg.
mkdir -p "$payload/lib/meraki/tools"
mv "$payload/lib/meraki/ffmpeg" "$payload/lib/meraki/tools/ffmpeg"
cp /usr/share/fonts/truetype/dejavu/*.ttf "$payload/share/fonts/truetype/dejavu/"
install -Dm644 /usr/share/doc/fonts-dejavu-core/copyright "$payload/share/licenses/dejavu.txt"
install -Dm644 "$project_root/packaging/flatpak/org.merakkicode.Meraki.desktop" "$payload/share/applications/org.merakkicode.Meraki.desktop"
install -Dm644 "$project_root/apps/desktop/src-tauri/icons/128x128@2x.png" "$payload/share/icons/hicolor/256x256/apps/org.merakkicode.Meraki.png"
install -Dm644 "$project_root/LICENSE" "$payload/share/licenses/meraki/LICENSE"
install -Dm644 "$project_root/NOTICE" "$payload/share/licenses/meraki/NOTICE"
install -Dm644 "$project_root/THIRD_PARTY_NOTICES.md" "$payload/share/licenses/meraki/THIRD_PARTY_NOTICES.md"
mkdir -p "$payload/lib/alsa-lib"
cp /usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so "$payload/lib/alsa-lib/"
cp /usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_ctl_pulse.so "$payload/lib/alsa-lib/"
cat > "$payload/etc/asound.conf" <<'ALSA'
pcm.!default { type pulse }
ctl.!default { type pulse }
ALSA
cat > "$payload/bin/meraki-desktop" <<'WRAPPER'
#!/bin/sh
export ALSA_CONFIG_PATH=/app/etc/asound.conf
export ALSA_PLUGIN_DIR=/app/lib/alsa-lib
exec /app/bin/meraki-desktop.real "$@"
WRAPPER
chmod +x "$payload/bin/meraki-desktop"
mkdir -p "$project_root/release-assets"
tar -czf "$project_root/release-assets/meraki-linux-payload.tar.gz" -C "$stage" payload
