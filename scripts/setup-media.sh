#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runtime="$project_root/tools/linux/ffmpeg"
build="$project_root/tools/linux/build"
mkdir -p "$runtime/bin" "$runtime/lib" "$runtime/licenses" "$build"
archive="$build/ffmpeg-7.1.1.tar.xz"
curl --fail --location --retry 3 https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz -o "$archive"
printf '%s  %s\n' 733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1 "$archive" | sha256sum --check -
tar -xf "$archive" -C "$build"
cd "$build/ffmpeg-7.1.1"
./configure --prefix="$runtime" --disable-doc --disable-debug --disable-autodetect \
  --enable-gpl --enable-libx264 --enable-libx265 --enable-libfreetype \
  --enable-libharfbuzz --enable-libfontconfig --enable-zlib \
  --disable-ffplay --enable-pic
make -j"$(nproc)"
make install
cp COPYING* LICENSE.md "$runtime/licenses/"
cp ffbuild/config.log "$runtime/licenses/build-configuration.txt"
for program in ffmpeg ffprobe; do
  mv "$runtime/bin/$program" "$runtime/bin/$program.real"
  # Resolve the complete dynamic dependency closure reported by the loader.
  while IFS= read -r library; do
    case "$(basename "$library")" in
      libc.so.*|libm.so.*|libpthread.so.*|libdl.so.*|librt.so.*|ld-linux*) continue ;;
    esac
    cp -L "$library" "$runtime/lib/"
  done < <(ldd "$runtime/bin/$program.real" | awk '/=> \// {print $3}')
  cat > "$runtime/bin/$program" <<'WRAPPER'
#!/bin/sh
runtime_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export LD_LIBRARY_PATH="$runtime_dir/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$0.real" "$@"
WRAPPER
  chmod +x "$runtime/bin/$program"
done
for package in libx264-dev libx265-dev libfreetype6 libharfbuzz0b libfontconfig1 zlib1g; do
  if [ -f "/usr/share/doc/$package/copyright" ]; then
    cp "/usr/share/doc/$package/copyright" "$runtime/licenses/$package.txt"
  fi
done
"$runtime/bin/ffmpeg" -version
