# Contributing to Meraki

Meraki Core Team welcomes focused improvements to editing, playback, accessibility and platform support.

Open an issue before a broad architectural change. Fork the repository, branch from main, and submit a small pull request describing the user-visible problem, the final behavior and validation. Include platform, graphics adapter and audio backend for multimedia changes. Never include private media, credentials or personal absolute paths.

## Quality checks

Run cargo fmt --all -- --check, cargo test --workspace --locked and cargo clippy --workspace --all-targets --locked -- -D warnings. In apps/desktop, run npm ci, npm test and npm run build.

Tests should verify invariants and behavior: rational time, revision conflicts, frame ordering, DSP continuity and encoded output. Hardware tests are explicitly opt-in; see docs/testing.md. Do not replace hardware evidence with a passing headless build.

Keep domain crates independent from the interface. Use bounded worker queues, preserve decimal-string IPC precision, and keep allocation, blocking locks and I/O out of the audio callback. Retain safe Rust boundaries and the CPU rendering fallback.

Use concise commit subjects such as feat(timeline): add marker navigation or fix(audio): reset playback epoch on seek. Keep third-party notices intact. By contributing original work, you agree to its distribution under Apache-2.0.

Maintained by Meraki Core Team and Meraki Contributors.
