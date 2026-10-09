# ctx-ppteer product roadmap

## Review scope

This review covers the README, Svelte frontend, Rust/Tauri backend, MCP server, bundled skill, and simulated frontend flows. No product code was changed. Native installation, cross-platform packaging, and real agent sessions still need end-to-end validation.

## Product today

ctx-ppteer is a compact Tauri desktop companion for coding-agent status. It watches one aggregate Markdown file, keeps the document visible in a small always-on-top window, and refreshes it when agents write changes. Agents publish project sections through MCP. Users can choose a source file, read rendered Markdown, edit the file, change appearance, save window placement, install the bundled skill, and register MCP configuration for supported agent harnesses.

The intended audience appears to be developers supervising coding agents. A primary persona of a developer switching between several projects or agent sessions is a hypothesis, because the repository contains no customer research or usage data.

## Core user journeys and current friction

### Connect an agent

The user opens Settings, installs a skill, and registers MCP. Skill installation and MCP registration are separate actions. The UI reports configuration presence, but does not prove that an agent can connect and publish a status. The README also describes menu behavior that does not exactly match the current implementation. Evidence: [Settings.svelte](src/Settings.svelte), [lib.rs](src-tauri/src/lib.rs), [README.md](README.md).

### Check progress

The user opens or glances at the viewer while an agent writes status updates. The backend tracks `health`, errors, and modification time, but the viewer only renders Markdown and does not expose that source state. A missing or unreadable file can therefore leave old content looking current. The frontend also queues updates while text is selected or edited. In a simulated flow, a queued update remained hidden after selection ended. Evidence: [app_state.rs](src-tauri/src/app_state.rs), [Viewer.svelte](src/Viewer.svelte), [app-types.ts](src/app-types.ts).

### Correct a note

The user clicks Edit or double-clicks the document, changes Markdown, and saves or cancels. In a simulated flow, Cancel left the abandoned draft in the next edit session. Human saves also carry no base-content or version check, so a newer agent update can be overwritten. Evidence: [Viewer.svelte](src/Viewer.svelte), `save_markdown` in [lib.rs](src-tauri/src/lib.rs).

### Monitor multiple projects

The MCP writer uses the project folder name as the section key and sorts sections alphabetically. Two unrelated checkouts with the same basename can replace one another. Separate MCP processes read and replace the aggregate file independently, so overlapping writes can lose acknowledged updates. These are code-derived risks, not reproduced production incidents. Evidence: [status_file.rs](src-tauri/src/status_file.rs), [mcp.rs](src-tauri/src/mcp.rs), bundled [SKILL.md](src-tauri/resources/skills/ctx-ppteer/SKILL.md).

## Product vision

Help developers supervising coding agents understand the latest reported work, blockers, and outcomes across local projects without repeatedly reopening chats.

This follows from the existing aggregate Markdown file, project sections, status-writing skill, MCP server, and persistent compact window. The product should become more trustworthy before it becomes more expansive. File freshness alone cannot establish that an agent is still running.

### Product principles

- Show uncertainty and source failures where users read statuses.
- Make setup end with a demonstrated update.
- Preserve acknowledged updates and expose editing conflicts.
- Keep reading compact, quiet, and compatible with ordinary Markdown.
- Add structured metadata only when it supports a concrete user outcome.

### Non-goals

- Agent execution or orchestration.
- A full task-management suite.
- Cloud collaboration or shared hosted state.
- A general-purpose Markdown editor.
- Exhaustive harness coverage before existing integrations work reliably.

## Highest-value use cases and user stories

### Established by the product

- As a developer, I want to see the latest agent-written project status in a persistent window, so that I can monitor work without reopening an agent chat.
- As a developer, I want to choose the Markdown source and adjust the viewer’s appearance, so that the status view fits my workspace.
- As an agent integration user, I want to write my project’s status through MCP, so that the viewer reflects work across sessions.

### Hypotheses to validate

- As a developer supervising several projects, I want to find the project that needs attention, so that I do not scan a long aggregate document.
- As a developer editing a status while agents run, I want conflicts surfaced before saving, so that I do not erase newer work.
- As a developer setting up a harness for the first time, I want to verify that a real status reaches the viewer, so that I know setup is complete.
- As a developer recovering from a bad update, I want to inspect and restore a recent version, so that useful context is not lost.
- As a developer leaving agents running in the background, I want an optional alert when a project needs input, so that I do not repeatedly check the window.

## Prioritization framework

The ranking weighs user value, strategic fit, effort, and uncertainty:

`score = value × strategic fit × confidence ÷ effort`

Value and fit use a 1–5 scale. Effort is 1 for small, 2 for medium, and 3 for large. Confidence is a judgment about evidence and feasibility, not measured ROI. Success targets below are proposed pilot thresholds because the repository supplies no usage baseline.

## Now

### 1. Show whether the displayed status is trustworthy

**Score:** 9.50. **Effort:** Small. **Confidence:** 95%.

**Problem and evidence.** The backend exposes health, errors, and modification time, but the viewer ignores them. Missing, deleted, invalid, oversized, or unreadable sources can leave old content looking current. Queued updates can also remain hidden after selection ends. Evidence: [app_state.rs](src-tauri/src/app_state.rs), [app-types.ts](src/app-types.ts), [Viewer.svelte](src/Viewer.svelte).

**User story and change.** As a developer checking agent progress, I want to see whether the displayed status is current or unreadable, so that I can decide whether to act on it. Add a quiet source-health indicator, last successful read/content-change time, useful empty states, and retry/source-selection actions. Keep file freshness distinct from agent liveness. Apply queued updates after selection ends.

**Dependencies and tradeoff.** Extend frontend snapshot types and keep source identity attached to cached content. Much of the backend data already exists, so this improves every reading session at low effort. It should precede navigation and alerts because those depend on trusted content.

**Acceptance criteria.** Missing, deleted, invalid UTF-8, oversized, and permission-denied files show an actionable state. Old content identifies its source. The newest queued update appears after deselection. Healthy viewing remains compact.

**Success indicator.** In a pilot, 90% of users correctly identify current versus stale/error content within five seconds, with zero missed queued updates in scenario checks.

### 2. Get one agent publishing successfully

**Score:** 3.83. **Effort:** Medium. **Confidence:** 85%.

**Problem and evidence.** Settings separates skill installation from MCP registration and never verifies an end-to-end connection. Registration checks configuration presence, not a working connection. Gemini skill and Cursor MCP support are also asymmetric. Evidence: [Settings.svelte](src/Settings.svelte), [lib.rs](src-tauri/src/lib.rs).

**User story and change.** As a first-time agent user, I want to connect my chosen agent and verify a status reaches the viewer, so that I know setup is complete. Add a per-harness setup checklist, explicit support matrix, connection test, and copyable verification prompt for a fresh agent session. Explain configured versus verified and when a restart is needed. Preserve existing configuration and back up replaced skill/config content.

**Dependencies and tradeoff.** Test the installed executable and MCP handshake. Actual publication remains an end-to-end check in a real harness. This has more integration work than item 1, but setup has little value if users never get an agent writing.

**Acceptance criteria.** A clean user profile can publish a visible sample update with one supported harness. Missing executables and malformed configs produce specific recovery guidance. Existing unrelated configuration survives. A failed check never reports verified.

**Success indicator.** In a pilot, 80% of first-time users publish a first status within five minutes without help. Record abandonment by setup step.

### 3. Preserve agent updates and human edits

**Score:** 3.17. **Effort:** Large. **Confidence:** 95%.

**Problem and evidence.** Each MCP process reads and replaces the entire aggregate file without interprocess coordination. Atomic replacement avoids partial files but cannot prevent lost updates. Human saves carry no base-content check, and Cancel retains the draft. Evidence: [status_file.rs](src-tauri/src/status_file.rs), [mcp.rs](src-tauri/src/mcp.rs), [lib.rs](src-tauri/src/lib.rs), [Viewer.svelte](src/Viewer.svelte).

**User story and change.** As a developer running agents while correcting a note, I want every acknowledged update preserved and conflicts shown, so that my overview does not silently lose work. Coordinate application-owned writers across processes. Check a content hash or base version before human saves. Keep drafts separate from displayed content. Cancel should restore the latest accepted text. On conflict, offer reload, keep draft, and explicit overwrite. Keep recovery copies bounded.

**Dependencies and tradeoff.** Requires a shared locking/write protocol in both the UI and MCP, content-based conflict checks, and Markdown-aware section handling. Arbitrary external editors cannot be forced to follow the protocol. This is technical work required for a user-facing trust outcome, not maintenance for its own sake.

**Acceptance criteria.** A controlled suite of 1,000 interleaved writes to distinct projects preserves all acknowledged final section updates. Same-content edit conflicts are surfaced. Cancel discards the draft and applies the newest queued snapshot. External changes detected before save are never silently overwritten.

**Success indicator.** Release gate: zero lost acknowledged updates in the controlled concurrency suite, and every simulated human/agent edit conflict is detected.

## Next

### 4. Give projects distinct identities

**Score:** 2.25. **Effort:** Large. **Confidence:** 75%.

**Problem and evidence.** The folder name is the section key. Two unrelated checkouts named `app` can replace the same section, while multiple sessions in one folder are currently aggregated. Evidence: [mcp.rs](src-tauri/src/mcp.rs), [status_file.rs](src-tauri/src/status_file.rs), bundled [SKILL.md](src-tauri/resources/skills/ctx-ppteer/SKILL.md).

**User story and change.** As a developer working across similarly named repositories, I want their statuses kept separate, so that I can tell which project needs attention. Add an optional stable project identifier alongside the display name, preserve the existing MCP contract, and keep the Markdown readable. Validate whether session-level separation is needed before adding it.

**Dependencies and tradeoff.** Update the skill and MCP together, migrate existing sections without duplication, and depend on protected writes from item 3. This matters greatly for affected users, but the frequency of duplicate names is unknown.

**Acceptance criteria.** Two paths with the same basename remain distinct across restarts. Old folder-name-only clients still work. Users can understand which checkout each section represents.

**Success indicator.** Zero cross-project replacements in identity tests and 90% correct project identification in a pilot with duplicate names.

### 5. Find the project that needs attention

**Score:** 1.73. **Effort:** Large. **Confidence:** 65%.

**Problem and evidence.** The default viewer is 360 × 220 and shows one scrolling document. Project ordering exposes only A–Z. There is no filtering, collapse, recent-update ordering, or archive flow. Evidence: [tauri.conf.json](src-tauri/tauri.conf.json), [Viewer.svelte](src/Viewer.svelte), [Settings.svelte](src/Settings.svelte), [status_file.rs](src-tauri/src/status_file.rs).

**User story and change.** As a developer monitoring several projects, I want to focus on relevant and recently changed notes, so that I can find the next thing to check without scanning the whole file. Add a project filter/jump control, collapse completed or inactive sections, and optional recent-update ordering. Archive explicitly rather than deleting. Preserve reading position when content refreshes.

**Dependencies and tradeoff.** Requires stable identities from item 4 and per-project content-change metadata. Whole-file modification time is insufficient. This should wait until multi-project use is confirmed.

**Acceptance criteria.** With ten project sections, users can find a named project and return to the full document. Unchanged content does not reorder itself. Collapsed/filter state and reading position survive refresh.

**Success indicator.** Halve median time to find a relevant project versus the current viewer without increasing errors.

## Later

### 6. Recover a recent overwritten note

**Score:** 1.40. **Effort:** Large. **Confidence:** 60%.

**Problem and evidence.** The product replaces current state and keeps no persistent history. Earlier context must be recovered from external backups or chat history. Evidence: [status_file.rs](src-tauri/src/status_file.rs), [lib.rs](src-tauri/src/lib.rs), [mcp.rs](src-tauri/src/mcp.rs).

**User story and change.** As a developer recovering from an incorrect status update, I want to inspect and restore a recent version, so that I can recover useful context. Offer bounded local per-project history and conflict-aware restoration. Set a small retention default and make storage and clear-history controls explicit.

**Dependencies and tradeoff.** Requires protected writes, project identities, bounded storage, and retention decisions. Defer until incident frequency is known.

**Acceptance criteria.** Restoring one project’s prior note does not change another project or silently overwrite a newer update. Retention limits are honored and data remains local.

**Success indicator.** 90% of seeded recovery tasks succeed within one minute. Measure real recovery demand before expanding retention.

### 7. Notify only when user action is needed

**Score:** 0.80. **Effort:** Large. **Confidence:** 40%.

**Problem and evidence.** The viewer has no typed blocked/completed state or notification flow. The bundled skill requests updates at material changes, but an unfocused user may miss them. Demand and reliable event semantics are unknown. Evidence: bundled [SKILL.md](src-tauri/resources/skills/ctx-ppteer/SKILL.md), [mcp.rs](src-tauri/src/mcp.rs), [Viewer.svelte](src/Viewer.svelte).

**User story and change.** As a developer leaving agents running in the background, I want an optional alert when a project needs input, so that I can respond without repeatedly checking the window. Add backward-compatible optional status fields, then opt-in per-project alerts for meaningful transitions. Include mute and quiet periods. Keep ordinary progress silent.

**Dependencies and tradeoff.** Requires identities, trustworthy metadata, validated status semantics, native permissions, and deduplication. Alerts have an interruption cost, so validate demand first.

**Acceptance criteria.** One alert is sent per real transition. Polling or restart does not duplicate alerts. Text-only progress edits do not alert. Users can mute alerts and reach the relevant note.

**Success indicator.** Halve response time to input-needed events while fewer than 10% of alerts are rated unnecessary.

## Recommended starting point

Start with these three updates:

1. **Show source health and recovery state.** It uses data the backend already produces and fixes the core trust problem for every user.
2. **Verify one real agent publishing path.** It turns setup from configuration work into a demonstrated first success.
3. **Protect concurrent writes and drafts.** It prevents silent data loss before the product encourages more projects, sessions, or history.

The third item is the largest effort. It should be treated as a release gate for broader multi-project use, even if items 1 and 2 ship independently.

## Important tradeoffs

Health beats navigation because every user relies on accurate content. Setup beats wider harness coverage because a registered config does not prove a useful connection. Protected writes come before encouraging more concurrent projects. Keep free-form Markdown as the primary artifact and add only the metadata needed for identity, freshness, conflicts, or validated notifications. History and alerts should follow observed incidents rather than become generic feature expansion.

## Validation plan

- Observe single-project and multi-project sessions to determine whether navigation is a real need.
- Run first-use setup sessions across the most-used harnesses, starting with a clean profile and a moved executable.
- Measure status-edit frequency and overlap with agent writes.
- Test whether users need last reported activity, agent liveness, or actionable blockers when they ask whether a status is current.
- Seed recovery and missed-input scenarios before prioritizing history or alerts.
- Decide whether identity should mean checkout, repository, project, or agent session. That choice affects navigation, history, and notifications.
