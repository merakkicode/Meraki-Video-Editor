<p align="center"><img src="screenshots/logo.jpg" width="128" alt="Meraki logo"></p>
<h1 align="center">Meraki Video Editor</h1>
<p align="center"><strong>Precision for your timeline. Room for your ideas.</strong></p>
<p align="center"><a href="../README.md">Português</a> · <strong>English</strong></p>

Meraki is a free, open-source desktop video editor for Windows and Linux, developed by **Meraki Core Team and Meraki Contributors**. Edit clips, animate titles, shape the image and sound, and deliver your video in one application. The current editor interface uses Portuguese workspace labels.

## Download version 1.0.0

| Platform | Official download |
|---|---|
| Windows 10/11 x64 | **[Meraki-Setup.exe](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0/Meraki-Setup.exe)** |
| Linux x64 | **[Meraki.AppImage](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0/Meraki.AppImage)** |

One package per platform, with the same editor features. No Rust, Node.js or separate FFmpeg installation is needed. Sources and checksums in the release are additional developer materials, not alternative installers.

Windows: run the installer. Linux: run `chmod +x Meraki.AppImage`, then `./Meraki.AppImage`. The Linux build baseline is Ubuntu 22.04; a graphical session and compatible drivers are required. Packages are unsigned. ARM and macOS are not included in this release.

If FUSE is unavailable, try `./Meraki.AppImage --appimage-extract-and-run`. See the [installation guide](installation.md) for requirements, checksum verification and troubleshooting.

## Four workspaces

| Workspace | Features |
|---|---|
| **Montagem — Edit** | Multitrack timeline, split, edge trim, per-track Ripple, magnetic snapping, markers, titles, transitions and linear/Bézier keyframes. |
| **Trama — Effects** | Node-based color and blur effects, GPU composition, transforms, opacity and Picture-in-Picture. |
| **Som — Audio** | Native playback, track/master faders, Mute/Solo, meters, biquad EQ and source waveforms. |
| **Entrega — Deliver** | Sequential render queue, YouTube/vertical/Web/Master presets and H.264, H.265 or ProRes output. |

720p proxies accelerate preview while exports use original media. Project files reference source files; relink media after moving it. Recovery checkpoints are saved every three minutes and offered on the next launch.

## First project

1. In **Montagem**, import a video into **Acervo** and insert it into the timeline.
2. Press **Space** to play, **Ctrl+B** to split, **N** to toggle snapping and **M** to add a marker.
3. Drag clip edges to trim. Adjust effects in **Trama** and sound in **Som**.
4. Save a `.meraki` project. In **Entrega**, choose the output, add the job to the queue and start rendering.

## Development

Rust 2021 · Tauri v2 · SolidJS · wgpu/WGSL · CPAL · FFmpeg.

[Build instructions](building.md) · [Architecture](architecture.md) · [Tests and qualification](testing.md) · [Contributing](../CONTRIBUTING.md) · [Report an issue](https://github.com/merakkicode/Meraki-Video-Editor/issues/new/choose).

CI builds and tests both platforms. Hardware-dependent audio/GPU tests run separately. Initial limitations include 8-bit composition without HDR, per-track Ripple and no automatic proxy-cache quota. Autosave complements manual saves; it does not replace backups. Performance depends on your hardware and media.

## License

Original Meraki code is [Apache-2.0](../LICENSE). Bundled dependencies retain their respective licenses and credits; see [third-party notices](../THIRD_PARTY_NOTICES.md). Corresponding distribution materials accompany the release.
