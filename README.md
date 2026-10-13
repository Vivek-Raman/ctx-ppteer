# ctx-ppteer

ctx-ppteer is a compact Tauri desktop status viewer. Agents send structured points through its MCP server, and the window refreshes when the JSON changes. Project headings, branch text, bullets, appearance, and always-on-top behavior retain the existing layout. Session IDs are stored but not displayed.

The MCP manages `~/.local/share/ctx-ppteer/status.json`, shared with the desktop app on all platforms. It works before the app is opened. Settings remain in `~/.config/ctx-ppteer/settings.json`. The settings page controls appearance, window behavior, project order, and agent integration. Old source-path and Markdown editor preferences are ignored. Existing Markdown files remain untouched and are no longer loaded.

## Status schema and MCP

The schema is defined in [status.schema.json](status.schema.json). The document has version `1` and an `entries` array. Each entry stores `folder_name`, nullable `session_id`, `branch`, `points`, and a server-generated RFC 3339 `updated_at` timestamp. Points contain plain `text`, `done`, and an optional HTTP or HTTPS `link`.

Both MCP tools require `folder_name` and a non-empty real `session_id`. `read_status` returns only the matching project's session record. `write_status` also accepts `branch` and the complete revised `points` list, replacing that record. Agents read first, then revise text, completion flags, and links. Omitted points are removed; superseded text is not retained as history. Anonymous writes and project-wide MCP reads are not supported. Existing records with a null session ID remain viewable for compatibility.

The supplied session ID identifies the record; it is not authentication of the calling harness. Writers hold an OS lock on a stable sidecar file for the entire read/modify/write operation, then atomically replace the JSON. Invalid JSON is reported and preserved. Documents are limited to 1 MiB.

The viewer renders points directly with Svelte and keeps the last valid view if a read fails. It groups by project and branch, with no session labels. Markdown rendering, source-file selection, and whole-document editing have been removed.

After upgrading the app, update installed copies of the skill through Settings and restart MCP clients to load the new tool arguments. The previous Markdown `status` argument is no longer supported.

## Development

```sh
npm install
npm run dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

To build a desktop bundle, install Rust and the Tauri prerequisites, then run `npm run tauri build`.
