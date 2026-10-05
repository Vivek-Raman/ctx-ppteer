# ctx-ppteer

ctx-ppteer is a small Tauri desktop viewer for one aggregate Markdown file. It keeps the document visible in a compact always-on-top window and refreshes it when agents write changes.

## Development

```sh
npm install
npm run dev
```

To build the frontend:

```sh
npm run build
```

To build a desktop bundle, install Rust and the Tauri prerequisites for the target platform, then run:

```sh
npm run tauri build
```

The current environment has no active Rust toolchain, so the native build still needs to be run on a configured macOS, Windows, and Linux machine. The frontend build and Tauri project inspection pass locally.
