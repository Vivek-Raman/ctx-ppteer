# Desktop Markdown viewer

Draft v0.2, 2026-10-03. This file is the authoritative design draft. Implementation has not started.

## Outcome and scope

Build a compact desktop application that watches a user-selected Markdown file and displays its current contents in a movable, resizable, always-on-top window. It should stay useful while the user works across eight codebase directories with multiple agents.

The application owns file observation and presentation. The external skill owns the document format, agent identification, and coordination between writers. The viewer never modifies the source document.

This directory was empty when inspected. There is no existing code, build system, or repository configuration to preserve.

## Decisions to settle

The user confirmed one aggregate file in one compact window and all three desktop platforms in the first release. Other recommendations remain open for iteration.

| Decision | Draft recommendation | Alternative and consequence |
| --- | --- | --- |
| File arrangement, confirmed | One explicitly selected aggregate file | Several selected files would keep each directory independent, but add source management. Outside the agreed first release |
| Window arrangement, confirmed | One compact window | Separate windows allow independent placement, but increase clutter and likely webview memory use. Outside the agreed first release |
| Release platforms, confirmed | macOS, Windows, and Linux from the first release | Requires native builds and real desktop testing on each before release |
| Meaning of lightweight | Measure executable size, installed footprint, process-tree memory, and idle CPU separately | Optimizing only binary size can hide substantial runtime costs |
| Workspace visibility | Pin above normal windows on the current workspace | Following all workspaces and fullscreen applications adds platform-specific behavior |

The aggregate file can contain any number of agent sections and can live outside the codebase directories. The app selects its exact path rather than scanning eight repositories for a special filename. Aggregating updates is the external producer's responsibility.

## Required behavior

1. First launch offers a native file picker. A path field also permits selecting a file that does not exist yet. Accept Markdown as UTF-8 text, with an optional UTF-8 BOM; do not require a particular filename or schema.
2. Read the selected file immediately. Watch it while the application remains open. Reflect completed local writes automatically, without a refresh button being necessary.
3. Render headings, paragraphs, lists, blockquotes, emphasis, inline code, fenced code, links, tables, strikethrough, and disabled task checkboxes. Preserve source order. Do not interpret agent names or status fields.
4. Default to always on top. Allow temporarily unpinning the window. File changes must not focus, raise, resize, or reposition it.
5. Allow dragging, resizing, scrolling, text selection, and copying. Keep controls limited to selecting the source, pinning, refreshing, appearance, and quitting.
6. Remember the selected path, size, position, pin state, theme, and text scale across launches. Restore the window inside a currently available monitor's work area.
7. Show a compact source label and file health. Distinguish waiting for a file, displaying current content, and showing the last readable content after an error.
8. Operate offline, without an account, service, background daemon, or telemetry. Exiting stops watchers and all application processes.

Close quits in the first release. Hide-to-tray, launch at login, global shortcuts, and click-through are optional later features. Read-only content remains interactive for scrolling and selection.

## Window and reading design

Start around 420 × 600 logical pixels, with a minimum around 280 × 200. These are prototype defaults to evaluate, not fixed dimensions. Use an opaque background, system fonts, compact spacing, and system light/dark preference. Avoid blur, transparency, bundled fonts, and animations initially.

Use native window decorations first. They provide dragging, resizing, and familiar close controls with less platform work. A custom slim title bar can follow if the native frame uses too much space.

Wrap prose. Give wide code blocks and tables horizontal scrolling. Disabled task checkboxes reflect the document; clicking them never writes to disk. Images display their alt text without loading an asset. Mermaid, math, syntax highlighting, embedded HTML, and remote media are outside the initial renderer contract.

On updates, keep the top visible block and its offset when it can be matched to the new document. Otherwise preserve and clamp the pixel scroll position. If the reader was at the bottom, keep them at the bottom. Do not scroll to a changed agent automatically. Defer replacing the content while a text selection is active, then render the latest snapshot when selection clears.

Display the file's modification time, labelled "File modified", rather than an implied agent heartbeat. An old document does not prove an agent stopped. Agent health, completion, and activity are whatever the Markdown says.

## Stack recommendation

Use Tauri 2 with a Rust backend and a small TypeScript, HTML, and CSS frontend built with Vite. No frontend framework is needed for one document and a few controls. Pin compatible stable dependency versions at implementation time and commit lockfiles.

Tauri uses platform webviews instead of bundling a browser engine. That makes it a strong candidate for a small application package, but runtime memory and rendering differ between operating systems. macOS uses WKWebView, Windows uses WebView2, and Linux uses WebKitGTK. [Tauri webview documentation](https://v2.tauri.app/reference/webview-versions/)

| Option | Advantages for this application | Costs and reason to choose it |
| --- | --- | --- |
| Tauri 2 | Rust file handling, system webview, desktop packaging and window APIs | Rust plus web tooling; webview differences and platform dependencies. Recommended starting point |
| Wails | Go backend, web frontend, an always-on-top window API | Similar system-webview tradeoffs. Prefer if Go is a stronger maintenance fit |
| Rust with egui/eframe | Native rendering without a system webview; one main language | Markdown fidelity, selection, accessibility, and styling need evaluation. Prototype if Tauri misses the memory budget |
| Electron | Bundled Chromium gives consistent web rendering and extensive desktop tooling | Bundles Chromium and Node.js. Poor fit for the slim-package priority |
| Separate native applications | Direct platform control and opportunity to minimize resources | Separate UI implementations and release work conflict with the cross-platform priority |

The comparisons are design judgments, not measured benchmarks. Framework capabilities come from [Wails window documentation](https://wails.io/docs/next/reference/runtime/window/), [egui's project documentation](https://github.com/emilk/egui), and [Electron's introduction](https://www.electronjs.org/docs/latest/).

Use Rust `notify` for filesystem events and `serde`/`serde_json` for settings and messages. Use `markdown-it` for CommonMark with selected extensions, and DOMPurify to sanitize generated HTML. Bundle assets locally. One small task-list renderer extension is sufficient. [markdown-it documentation](https://markdown-it.github.io/markdown-it/), [DOMPurify documentation](https://github.com/cure53/DOMPurify)

Use the native dialog plugin for picking files. Add an opener capability only for explicit HTTP/HTTPS link clicks. Implement settings with a small versioned JSON file in the application's OS config directory. No database, general filesystem plugin, HTTP server, or agent integration is needed.

## File observation and update flow

The configured path is the document's identity. An editor replacing that path should work; moving the document somewhere else requires selecting its new location.

1. Register a nonrecursive watch on the parent directory before the initial read. Filter events to the selected document. If an event is ambiguous, recheck that document rather than trusting the event type.
2. Events mark the document dirty. Debounce for approximately 150 ms, with a 500 ms maximum wait under continuous events. Run reads outside the UI thread.
3. Read at most 1 MiB plus one byte, so a growing file cannot defeat the size limit. Reject oversized files, nonregular files, and invalid UTF-8. Check metadata around the read and retry if it changed.
4. For transient errors or changing content, retry after roughly 100, 250, and 500 ms. Keep the previous successful document during retries. A stable empty file is valid and eventually clears the view.
5. Compare bytes to the current snapshot. Skip rendering unchanged content, but update file metadata if necessary. Keep only the current snapshot and bounded transient buffers.
6. Publish a source ID, selection generation, monotonically increasing revision, Markdown, modification time, and health state. Ignore late reads and messages belonging to an old path selection or older revision.
7. The frontend subscribes before fetching its initial snapshot. Apply revision ordering so an initial fetch cannot overwrite a newer event. Fetch the latest snapshot again on reload or return from suspension.

Watch parent directories because saving may truncate or replace a file. Network filesystems may not produce native events, and a deleted parent needs special recovery. These are documented limitations of [notify](https://docs.rs/notify/latest/notify/).

Use a shared 30-second reconciliation tick to recheck the selected path and repair missing watches. Recheck immediately on resume and manual refresh. Missing files or parents use a low-frequency retry, around two seconds, against the configured path and nearest existing parent. Do not recursively watch a repository or root directory.

Offer an explicit two-second content-polling mode for mounts with unreliable notifications. Native events remain the default. Native mode can take up to the reconciliation interval to recover a silently missed event; polling mode trades a bounded update delay for more filesystem work.

Coalesce event bursts with a bounded queue or dirty flag. A slow renderer receives the latest revision rather than a growing history. If several sources are chosen later, share watches for common parent directories and update only the changed source.

## Edge cases and recovery

| Situation | Expected behavior |
| --- | --- |
| File absent at startup | Show "Waiting for file" and the selected path; render when it appears |
| Atomic rename over the file | Render the replacement and continue observing future replacements |
| Delete and recreate | Keep previous content with a missing-file notice; recover automatically |
| Parent directory removed | Enter recovery mode; reattach when the configured path becomes available |
| Permission denied or sharing lock | Retry transient failures; show a readable error and retain prior content |
| Truncated or partially written Markdown | Debounce and retry detectable changes; render readable incomplete Markdown if that is the stable file |
| Several agents write concurrently | Show the latest readable bytes. The viewer cannot recover overwritten updates or guarantee a coherent multiwriter snapshot |
| File grows beyond 1 MiB | Stop rendering new content, show the limit, retain prior content; recover when it shrinks |
| Invalid encoding | Report that UTF-8 is required; recover on the next valid write |
| Symlink selected | Initially resolve to its regular-file target, show and persist that target. Retargeting the symlink requires reselection in v1 |
| Cloud or network storage stalls | Keep UI responsive, bound queued work, show an unavailable-source state; latency guarantees apply to local storage |
| Sleep and wake | Reconcile immediately and restore watches if needed |
| Monitor unplugged or scaling changed | Clamp restored geometry to a visible work area using logical dimensions |
| Broken settings file | Use defaults and preserve the broken file for diagnosis; do not crash |
| Settings save fails | Continue with session settings and show a compact warning |
| Rapidly changing source selection | Cancel or discard old work; never display the previous file under the new label |

There is no reliable "finished writing" signal for arbitrary writers. Stability checks reduce flicker but cannot prove consistency. The external producer should use coordinated writes and atomic replacement where possible. This is an integration constraint, not work on the skill.

## Rendering and local access rules

Treat the Markdown as content even though it is local. Disable raw HTML, sanitize the rendered result, remove image/embed elements, and disallow scriptable URLs. Render relative paths as text initially. Allow document anchors and explicitly clicked HTTP/HTTPS links; open external links in the system browser. Never navigate the application webview to document links.

Apply a restrictive content security policy that permits bundled assets and the IPC mechanism Tauri requires, while blocking remote images, frames, and connections. The frontend receives document snapshots through narrow commands; it does not receive arbitrary file-read or shell access. Rust validates command arguments and permits reads only for the configured regular-file target.

Limit window/dialog/opener capabilities to the application window. Tauri capabilities govern available IPC permissions; custom Rust commands still need explicit argument validation. [Tauri capabilities](https://v2.tauri.app/security/capabilities/)

Do not log document contents. Routine logs contain error codes and source IDs; detailed path diagnostics stay local and opt-in. Persist settings atomically with a schema version. Persist document contents only in memory.

## Cross-platform contract

Always on top means above ordinary application windows on the current desktop. It does not guarantee visibility above secure OS screens, exclusive fullscreen applications, or system UI. Test real desktop behavior; a successful API call alone is insufficient. Tauri exposes [always-on-top window control](https://v2.tauri.app/reference/javascript/api/namespacewindow/#setalwaysontop).

| Platform | Initial contract and work to verify |
| --- | --- |
| macOS | Test pinning, focus, resizing, Spaces, sleep, and screen scaling. Visibility in other Spaces or fullscreen Spaces is a separate optional requirement |
| Windows | Test normal-window pinning, WebView2 installation, file sharing locks, and mixed-DPI monitors |
| Linux | Select supported distributions and desktop environments. Test X11 and Wayland separately; advertise pinning only where verified, since window placement and stacking depend on the desktop |

Tauri's all-workspaces API is unsupported on Windows. Do not promise workspace parity across platforms. [Tauri workspace API](https://v2.tauri.app/reference/javascript/api/namespacewindow/#setvisibleonallworkspaces)

Use native release builds for each OS. macOS distribution needs signing and notarization for a smooth install; Windows signing is a distribution decision. Linux requires deciding between dependency-based packages and larger portable bundles. A slim executable is not a universally self-contained binary. Development prerequisites include native toolchains and Linux WebKitGTK dependencies. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Proposed resource budgets

These are targets to evaluate in the first release prototype, not existing measurements or framework guarantees.

| Metric | Initial target and measurement |
| --- | --- |
| Executable | At most 15 MiB per architecture, stripped release build |
| Installed app assets | At most 30 MiB, excluding shared system runtimes; report installer and downloaded runtime sizes separately |
| Idle CPU | Mean below 0.5% of one core over five minutes after warmup, including attributable webview processes |
| Memory | Target at most 150 MiB across app and attributable webview processes with a 100 KiB document; report platform measurement method and GPU/shared memory separately |
| Normal update | p95 within 500 ms of a completed local write to visible content under native watching, for a 100 KiB document |
| Cold startup | Usable content within two seconds on the named reference machine with runtime already installed |
| Sustained writes | Remain responsive at ten writes per second for a minute; settle to final content within one second after writes stop |

Also stress a 1 MiB aggregate document containing sections for sixteen agents across eight directories. Record hardware, OS, runtime versions, artifact sizes, and baseline versus stress measurements on all three platforms.

Start with release stripping, link-time optimization, and size-oriented compilation. Compare size and latency before choosing final optimization flags. [Tauri size guidance](https://v2.tauri.app/concept/size/)

If memory exceeds the budget materially, measure an egui prototype with the same fixture before investing in UI polish. A few extra frontend dependencies are unlikely to explain a large system-webview baseline. The prototype must resolve that tradeoff with numbers.

## Implementation sequence

| Slice | Deliverable | Completion evidence |
| --- | --- | --- |
| 1. Prove the desktop boundary | Tauri release builds open one fixed fixture, render Markdown, stay pinned, and update after direct write and atomic replacement on macOS, Windows, and Linux | Real desktop checks on all three platforms; record size, process-tree memory, CPU, and update latency. Define the supported Linux desktops before claiming pinning support |
| 2. Make the source recoverable | Source picker/path entry, bounded reads, debouncing, recovery, revisions, and source health | Integration tests cover file replacement, disappearance, bad encoding, oversized input, event bursts, and late reads |
| 3. Make it usable all day | Pin toggle, geometry/settings persistence, theme, text scale, scroll preservation, and safe links | Restart, monitor removal, selection/copy during updates, keyboard use, and malicious-content checks |
| 4. Package supported platforms | Native build jobs and platform-specific installation instructions | Install and smoke-test actual artifacts on every advertised OS/desktop combination |

Proposed files are `src/main.ts`, `src/markdown.ts`, `src/styles.css`, `src-tauri/src/main.rs`, `watcher.rs`, `source.rs`, `settings.rs`, `src-tauri/tauri.conf.json`, and a narrowly scoped capability file. Keep one source implementation initially; avoid an extensible plugin architecture.

No data migration or source rollback is required because the app only reads Markdown. A rejected prototype can be replaced without changing producer files. Keep settings versioned, preserve unsupported future-version settings, and provide an explicit reset action later if needed.

## Verification and acceptance

When implementation exists, run `npm run check`, `npm run build`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `npm run tauri build`. Define the frontend check script during setup. Rust commands run from `src-tauri`.

Acceptance requires these observable outcomes:

- A fixture containing the supported Markdown constructs renders correctly at narrow and wide window sizes. Code and tables remain readable without expanding the window.
- Direct writes and at least twenty atomic replacements continue updating without reopening the app. Initial-read/event races never revert the view.
- Missing files, deleted parents, permission failures, invalid UTF-8, oversized input, and valid empty content produce the specified states and recover.
- Rapid writes and rapid source switching cannot create an unbounded queue, freeze controls, or display content from a deselected path.
- The app never changes the source, including its modification time. Selection and copy work while updates arrive.
- HTML, event handlers, script URLs, embedded media, and remote images cannot execute code or trigger automatic network requests. Explicit approved links open outside the webview.
- Pinning stays effective above ordinary windows without focus theft. Restart restores settings; unplugging a monitor never leaves the window unreachable.
- Sleep/resume and manual refresh reconcile the latest file. Native watcher failure enters recovery; explicit polling works on the chosen test mount.
- Resource measurements meet the agreed budgets, or the spec records an accepted revision before release. Closing leaves no background application process.

Unit tests cover bounded reading, revision handling, and renderer rules. Temporary-directory integration tests cover filesystem recovery. Desktop smoke checks cover window-manager behavior, which browser previews cannot prove. Cross-platform support requires checks on those platforms; macOS-only testing is insufficient.

## Remaining choices for iteration

1. Which minimum OS versions, Linux distributions, and desktop environments must the first release support?
2. Must the window follow every workspace or appear alongside fullscreen applications?
3. Are the proposed size and memory budgets small enough, or is runtime memory the overriding constraint?

The supported platform matrix determines the release test scope. Workspace behavior and resource budgets can be evaluated through the first prototype before committing to extra platform work.
