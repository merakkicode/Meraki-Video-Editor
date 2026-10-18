# Meraki Desktop

Tauri v2, SolidJS and TypeScript connect four creative workspaces to the Rust engine. See the root README for platform dependencies and docs/architecture.md for ownership and scheduling.

| Command | Purpose |
|---|---|
| npm run dev | Start Vite and the native engine |
| npm run dev:web | Frontend without native media services |
| npm test | Precision, snapping and trim tests |
| npm run build | Type checking and frontend build |
| npm run bundle:windows | Windows NSIS and MSI |
| npm run bundle:linux | Linux AppImage and Debian |

Platform configuration is merged from src-tauri/tauri.windows.conf.json or tauri.linux.conf.json. Prepare the platform runtime before bundling. Packages are under target/release/bundle at the workspace root.

Space plays/pauses; Ctrl+B splits; Ctrl+Z undoes; Ctrl+S saves; Ctrl+O opens. V selects, C cuts, N toggles snapping and M adds a marker. Drag edges to trim, holding Shift before the gesture for per-track Ripple. Media context menus provide proxies and reconnection.

Snapshots contain metadata and decimal-string times/IDs. Pixels use a bounded binary channel and WebGL2 presentation. Audio and offline rendering remain native services.
