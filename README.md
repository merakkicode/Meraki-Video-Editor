<p align="center"><img src="docs/screenshots/logo.jpg" width="140" alt="Meraki logo" style="border-radius: 12px;"></p>
<h1 align="center">Meraki Video Editor</h1>
<p align="center"><strong>Precision for your timeline. Room for your ideas.</strong></p>
<p align="center">Edit · Compose · Mix · Deliver</p>

<p align="center">
  <a href="README.md">🇺🇸 English</a> ·
  <a href="README.pt-BR.md">🇧🇷 Português</a>
</p>

<p align="center">
  <img alt="Version" src="https://img.shields.io/badge/version-v1.0.0--rc1-9D4EDD">
  <img alt="License" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-00D2FF">
  <img alt="Windows reference tests" src="https://img.shields.io/badge/reference%20tests-74%2F74%20passing-2ea44f">
  <img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-dea584">
  <img alt="Frontend" src="https://img.shields.io/badge/frontend-Tauri%20v2%20%2B%20SolidJS-00D2FF">
  <img alt="Windows x64" src="https://img.shields.io/badge/Windows-x64-0078D4">
  <img alt="Linux x64 build target" src="https://img.shields.io/badge/Linux-x64%20target-FCC624">
</p>

Meraki is a desktop non-linear video editor built for creators who want precise control without losing their creative flow. A Rust engine, rational timeline and WGSL compositor sit behind a focused dark interface with a cyan/violet visual language.

**Built and maintained by Meraki Core Team and Meraki Contributors.**

> **Release candidate:** the Windows reference suite passed 74 tests. Linux packaging and CI targets are configured; Linux runtime qualification and Flathub approval are pending. Download links become available when the tag workflow succeeds. Packages are currently unsigned.

## Four creative workspaces

### 1. Montagem (Edit)
<p align="center">
  <img src="docs/screenshots/montagem.jpg" width="100%" alt="Meraki Montagem Workspace" style="border-radius: 8px;">
</p>

Multi-track video and audio editing, sub-frame rational accuracy, interactive Trim/Ripple, magnetic Snap, timeline markers, keyframes, transitions and transparent titles.

### 2. Trama (Effects & Nodes)
<p align="center">
  <img src="docs/screenshots/trama.jpg" width="100%" alt="Meraki Trama Workspace" style="border-radius: 8px;">
</p>

Typed effect graph with color correction (brightness, contrast, saturation), GPU blur, bypass and alpha composition. Picture-in-picture with scale, position and opacity animation.

### 3. Som (Audio)
<p align="center">
  <img src="docs/screenshots/som.jpg" width="100%" alt="Meraki Som Workspace" style="border-radius: 8px;">
</p>

Native low-latency playback (~31.7 ms output latency on WASAPI, zero underruns), track faders, Mute/Solo, peak/RMS meters, biquad EQ and real waveforms. WASAPI on Windows; ALSA with PipeWire/PulseAudio routing on Linux.

### 4. Entrega (Deliver)
Sequential export with render queue, cancellation and progress. YouTube 1080p/4K, vertical social 9:16, web and master presets using H.264, H.265 or ProRes.

The Windows audio reference run measured **31.69 ms estimated output latency** with zero underruns. Latency depends on the device, driver and buffers; Linux has not yet been measured. See [test evidence and limitations](docs/testing.md).

## Download and install

Release channel: [v1.0.0-rc1](https://github.com/merakkicode/Meraki-Video-Editor/releases/tag/v1.0.0-rc1). Check SHA256SUMS before installing.

| Platform | Release asset |
|---|---|
| Windows x64 | [Meraki-Setup.exe](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki-Setup.exe) · [Meraki.msi](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.msi) |
| Linux x64 | [Meraki.AppImage](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.AppImage) · [meraki.deb](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/meraki.deb) |

**Windows:** run NSIS or MSI. WebView2 is required; the installer can download its bootstrapper. GPU rendering supports DirectX 12/Vulkan with CPU fallback.

**AppImage:**

```sh
chmod +x Meraki.AppImage
./Meraki.AppImage
```

The baseline is Ubuntu 22.04-compatible glibc. A display, graphics driver and audio service are still required. Without FUSE, use `./Meraki.AppImage --appimage-extract-and-run`.

**Debian / Ubuntu:**

```sh
sudo dpkg -i meraki.deb
# Resolve missing system dependencies if needed:
sudo apt-get -f install
```

**Flatpak / Flathub:** submission is pending. Once approved and published:

```sh
flatpak install flathub org.merakkicode.Meraki
flatpak run org.merakkicode.Meraki
```

Until then, use the local manifest through the [packaging guide](docs/releasing.md). The sandbox requests graphics, audio and host-file access for local media editing.

## Build from source

Use current stable Rust and Node.js 24. The workspace uses Rust 2021; lockfiles record dependencies. FFmpeg runs as an external process; release packages include platform-specific tools and notices.

### Windows

Install Visual Studio C++ Build Tools with Windows SDK, Rust MSVC, Node.js and WebView2.

```powershell
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
Set-Location meraki
./scripts/setup-media.ps1
cargo test --workspace --locked
Set-Location apps/desktop
npm ci
npm test
npm run dev
```

Build NSIS and MSI from the repository root with `./scripts/build-windows.ps1`.

### Linux / Ubuntu 22.04

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl file libwebkit2gtk-4.1-dev \
  libgtk-3-dev libasound2-dev libasound2-plugins libssl-dev librsvg2-dev \
  patchelf libvulkan-dev fonts-dejavu-core nasm yasm libx264-dev libx265-dev \
  libfreetype6-dev libharfbuzz-dev libfontconfig1-dev zlib1g-dev
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd meraki
bash scripts/setup-media.sh
export MERAKI_FFMPEG_DIR="$PWD/tools/linux/ffmpeg/bin"
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

The Linux script builds checksum-pinned FFmpeg 7.1.1. Older distribution packages may lack timestamp options required by the pipeline. Run `bash scripts/build-linux.sh` at the repository root for AppImage and Debian packages. PipeWire systems need ALSA compatibility configuration; Flatpak uses the PulseAudio-compatible socket.

### Quality checks

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Run `npm test` and `npm run build` in `apps/desktop`. [Hardware/media tests](docs/testing.md) are opt-in; hosted CI does not claim to exercise physical audio hardware.

## Workspace crates

| Crate | Responsibility |
|---|---|
| `meraki-core` | Rational time, frame rates, SMPTE timecode and typed IDs. |
| `meraki-timeline` | Tracks, clips, source mapping, trims, markers and animation. |
| `meraki-graph` | Typed effect DAG, cycle detection and execution ordering. |
| `meraki-engine` | Single-writer transactions, snapshots, revision checks and bounded Undo. |
| `meraki-media` | Probe, decoding, frame cache, PTS mapping and preview proxies. |
| `meraki-audio` | CPAL, preallocated SPSC transport, mixing, gain smoothing, meters and EQ. |
| `meraki-compositor` | Shared wgpu/WGSL and CPU composition, effects and title rasterization. |
| `meraki-project` | Persistence, migration, relink and recovery journals. |
| `meraki-render` | Offline frame iteration, encoding, queue and cancellation. |
| `meraki-desktop` | Tauri commands, transport coordination, dialogs and lifecycle. |

Read the [architecture](docs/architecture.md) for thread ownership, clocks and pixel flow. Preview proxies never replace original sources in delivery. GPU initialization failure selects the CPU fallback.

## Community and license

Contributions to creative workflows, accessibility, correctness and performance are welcome. Start with [CONTRIBUTING](CONTRIBUTING.md), follow the [Code of Conduct](CODE_OF_CONDUCT.md), and use issue templates for reproducible bugs or focused requests. Security reports belong in [private reporting](SECURITY.md).

Meraki's original code is **MIT OR Apache-2.0**, at your option. See [LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Dependencies, fonts and FFmpeg retain their respective licenses and authorship; consult [third-party notices](THIRD_PARTY_NOTICES.md).
