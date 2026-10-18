# Testing and platform qualification

The Windows reference run completed 74 tests: 59 regular Rust tests, 8 hardware/media integration tests and 7 TypeScript tests. This is a recorded reference result, not a claim that every platform has completed qualification.

## Standard checks

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cd apps/desktop
npm ci
npm test
npm run build
```

Hosted CI runs the regular suite on Windows and Ubuntu 22.04, then builds platform packages. Integration tests requiring a physical audio device or hardware GPU remain opt-in. Tests are not silently skipped to represent a hardware pass.

## Hardware/media integration

Prepare FFmpeg and fixtures using scripts/setup-media.ps1, scripts/create-fixtures.ps1 and scripts/create-audio-fixtures.ps1 on Windows. Then run:

```sh
cargo test --workspace --locked -- --ignored --nocapture --test-threads=1
```

The suite covers decoded PTS/seek behavior, real PCM and audio clock, GPU parity, encoded output, animation, titles, transitions, cancellation, proxies and relink. In the proxy test, a deliberately blue proxy is presented while a red original is exported, proving that delivery retains original media.

Reference Windows measurements on an RTX 3050 6 GB Laptop GPU: GPU pipeline p95 7.83 ms at 1080p; estimated CPAL output latency 31.69 ms with zero underruns during the test. Decode plus GPU delivered 146/150 distinct 60 Hz samples. These figures are bounded test results, not universal performance guarantees or a Linux audio measurement.

## Manual acceptance

1. Import media; verify playback, pause, seek and audio sync.
2. Drag head/tail trim handles; check source limits and one-step Undo. Hold Shift before the gesture for Ripple on that track.
3. Toggle N; approach another clip edge or marker and verify a frame-aligned snap. M creates a marker; double click changes its name/color.
4. Generate a proxy from the media context menu, enable Use Proxies and export. Confirm output uses originals.
5. Move a disposable source file and reconnect it through the offline media menu; check timing and stream validity.
6. Make an unsaved edit, wait for the autosave notice, terminate only the test process and reopen. Recover and save a new project copy.
7. Install/uninstall each package on a clean OS; verify Vulkan, default audio routing, dialogs, Unicode paths, scaling and title fonts.

The Windows reference run includes native trim/marker/proxy interactions and recovery after forced process termination. Linux and Flatpak runtime qualification must be completed on their target systems before claiming production support. Windows packages are unsigned until signing is configured.

Known limits: one application instance at a time; autosave can lose edits after the latest checkpoint; proxy disk cache has no automatic quota; Ripple is per-track; relink resets history; no HDR workflow; ProRes output retains the compositor's 8-bit precision. Trims incompatible with an existing transition are rejected instead of silently moving that transition.
