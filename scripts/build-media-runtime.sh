#!/usr/bin/env bash
# Build the exact media runtime and retain corresponding sources for distribution.
set -euo pipefail
target="${1:?Usage: build-media-runtime.sh linux|windows}"
case "$target" in linux|windows) ;; *) exit 2 ;; esac
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$root/tools/media-build/$target"
prefix="$work/prefix"
sources="$work/sources"
mkdir -p "$prefix" "$sources" "$root/release-assets"
fetch() {
  curl --fail --location --retry 3 "$1" -o "$sources/$2"
  printf '%s  %s\n' "$3" "$sources/$2" | sha256sum --check -
  tar -xf "$sources/$2" -C "$work"
}
fetch https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz ffmpeg-7.1.1.tar.xz 733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1
fetch https://download.videolan.org/pub/videolan/x264/snapshots/x264-snapshot-20191217-2245-stable.tar.bz2 x264.tar.bz2 b2495c8f2930167d470994b1ce02b0f4bfb24b3317ba36ba7f112e9809264160
fetch https://download.videolan.org/videolan/x265/x265_3.6.tar.gz x265.tar.gz 663531f341c5389f460d730e62e10a4fcca3428ca2ca109693867bc5fe2e2807
cross=()
cmake_cross=()
ff_cross=()
exe=''
if [[ "$target" == windows ]]; then
  cross=(--host=x86_64-w64-mingw32 --cross-prefix=x86_64-w64-mingw32-)
  cmake_cross=(-DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++)
  ff_cross=(--target-os=mingw32 --arch=x86_64 --cross-prefix=x86_64-w64-mingw32- --enable-cross-compile)
  exe='.exe'
fi
cd "$work/x264-snapshot-20191217-2245-stable"
./configure --prefix="$prefix" --enable-static --disable-cli --enable-pic "${cross[@]}"
make -j"$(nproc)"
make install
cmake -S "$work/x265_3.6/source" -B "$work/x265-build" "${cmake_cross[@]}" \
  -DCMAKE_INSTALL_PREFIX="$prefix" -DCMAKE_BUILD_TYPE=Release -DENABLE_SHARED=OFF \
  -DENABLE_CLI=OFF -DENABLE_NUMA=OFF -DENABLE_LIBNUMA=OFF -DENABLE_ASSEMBLY=ON
cmake --build "$work/x265-build" -j"$(nproc)"
cmake --install "$work/x265-build"
cd "$work/ffmpeg-7.1.1"
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
./configure --prefix="$prefix" --disable-doc --disable-debug --disable-autodetect \
  --disable-ffplay --disable-network --enable-gpl --enable-libx264 --enable-libx265 \
  --enable-static --disable-shared --pkg-config-flags=--static \
  --extra-cflags="-I$prefix/include" --extra-ldflags="-L$prefix/lib -static-libgcc -static-libstdc++" \
  --extra-libs='-lstdc++ -lm' "${ff_cross[@]}"
make -j"$(nproc)"
make install
runtime="$work/runtime"
mkdir -p "$runtime/bin" "$runtime/licenses"
cp "$prefix/bin/ffmpeg$exe" "$prefix/bin/ffprobe$exe" "$runtime/bin/"
if [[ "$target" == windows ]]; then
  # A fully static runtime must not depend on DLLs from the build toolchain.
  x86_64-w64-mingw32-objdump -p "$runtime/bin/ffmpeg.exe" > "$sources/pe-dependencies.txt"
  if grep -Ei 'DLL Name:.*(libstdc|libgcc|libwinpthread)' "$sources/pe-dependencies.txt"; then
    echo 'Unexpected toolchain DLL dependency' >&2; exit 1
  fi
fi
cp COPYING* LICENSE.md "$runtime/licenses/"
cp "$work/x264-snapshot-20191217-2245-stable/COPYING" "$runtime/licenses/x264-COPYING"
cp "$work/x265_3.6/COPYING" "$runtime/licenses/x265-COPYING"
cp ffbuild/config.log "$sources/ffmpeg-config.log"
cp "$work/x265-build/CMakeCache.txt" "$sources/x265-CMakeCache.txt"
cp "$work/x264-snapshot-20191217-2245-stable/config.mak" "$sources/x264-config.mak"
cp "$0" "$sources/build-media-runtime.sh"
cp -r "$runtime/licenses" "$sources/"
dpkg-query -W gcc g++ gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 cmake nasm pkg-config > "$sources/toolchain.txt"
tar -czf "$root/release-assets/media-runtime-$target.tar.gz" -C "$runtime" .
tar -czf "$root/release-assets/media-sources-$target.tar.gz" -C "$sources" .
