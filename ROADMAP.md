# ctx-ppteer product audit and roadmap

Reviewed 2026-10-20 against `main` at `4f822ee`, version `0.2.0`. This replaces the earlier Markdown-viewer audit.

## Recommendation

Keep ctx-ppteer focused on a reliable personal overview of coding-agent work. The refactor improves normal concurrent writes, session isolation, rendering, dismissal, and settings behavior. The highest priorities are now crash recovery, understandable freshness and empty states, and setup that proves an agent can publish.

Do not carry the old Markdown editing and whole-document conflict roadmap forward. Those flows were removed. Retention safety and project/session attribution are the next product decisions.

## Scope and evidence

Reviewed the frontend, Rust domain/store/watcher, desktop commands, preferences, integration adapters, schema, bundled skill, README, and tests. Inspected the actual Svelte viewer and Settings through isolated browser IPC fixtures. Product code and saved statuses were not changed.

Verification completed:

- `npm test` passed: 20 controller, five lifecycle, and six retention tests, plus Svelte checks, TypeScript checks, and production build.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed all 37 tests.
- `npm run format:check` passed.
- The documented browser fixture passed all 15 motion checks, including ordering changes, parallel dismissal, failed dismissal, returning rows, and reduced motion.
- Additional isolated UI checks confirmed an empty waiting state with no explanation, cached reports remaining visible without a warning after a waiting snapshot, and two sessions merging under one project/branch.
- A temporary Rust probe linked to the compiled library confirmed that a preexisting sidecar lock blocks a write beyond a bounded 500 ms observation; releasing that synthetic lock lets the write complete. The unbounded wait is established by the loop in the source.
- A second temporary-store probe confirmed that editing a point while retaining its ID preserves its original submitted time and that pruning can remove that recently edited unfinished point.

Limitations: fixture checks do not verify native window placement, real harness discovery, fresh-session skill invocation, external-link opening, installer behavior, or Windows/Linux operation. No user interviews or usage baseline were supplied. A routine live MCP status write did not return; the cause of that live call was not established and was not used to prove the lock finding.

Evidence labels below mean:

- **Observed:** directly inspected behavior or a reproduced scenario.
- **Code-derived:** a consequence of implementation, without a full production reproduction.
- **Hypothesis:** user need, prevalence, or impact still requires validation.

## Product today

ctx-ppteer is a local desktop status board. Agents publish structured plain-text points through MCP. Each project/session record contains a branch and revised point list; points have stable IDs, completion flags, optional HTTP/HTTPS links, and submission timestamps. The viewer groups points by project and branch, hides session labels, and updates as the JSON changes.

The MCP manages its own fixed status path and works before the viewer is opened. Users can dismiss individual points, enable age-based auto-clear, choose project ordering, change theme/color/text size, pin the viewer, reset preferences, and install skills and MCP registration for supported harnesses.

It no longer loads arbitrary Markdown, offers source-file selection, or provides an editor. The current structure separates frontend viewer/settings/integrations/shared code and Rust status/preferences/desktop/integrations code. That organization makes targeted changes easier, but it is an implementation improvement rather than a separate user benefit.

Sources: [README](README.md), [schema](status.schema.json), [MCP contract](src-tauri/src/mcp.rs), [viewer](src/viewer/Viewer.svelte), [preferences](src-tauri/src/preferences/mod.rs).

## What changed since the previous audit

| Earlier finding | Current assessment | Evidence |
| --- | --- | --- |
| Whole-file writes lose updates across independent agents | Addressed for cooperating writers during normal operation through a shared lock and atomic replacement. Crash recovery remains a defect. | [store](src-tauri/src/status/store.rs), concurrency tests |
| Cancel retains an abandoned Markdown draft | Retired. The editor and whole-document save flow were removed. | [README](README.md), [desktop commands](src-tauri/src/desktop/commands.rs) |
| Queued update disappears after text selection | Fixed in the controller and covered by a passing selection-release test. | [controller](src/viewer/controller.ts), [controller tests](tests/viewer-controller.test.ts) |
| Backend errors are invisible | Partly fixed. Error strings render, but health is ignored, missing-file waiting has no error, and errors appear after all rows. | [snapshot](src-tauri/src/status/snapshot.rs), [viewer](src/viewer/Viewer.svelte) |
| Folder names overwrite unrelated agent records | Improved by project/session keys. Distinct sessions retain separate records, but same-name projects still merge visually. Same folder/session pairs remain one record by contract. | [domain](src-tauri/src/status/domain.rs), [rows](src/viewer/rows.ts) |
| Only alphabetical sorting is available | Fixed. Newest first, oldest first, and A–Z are implemented. Search, filtering, and user-controlled collapse remain absent. | [retention settings](src/settings/RetentionSettings.svelte), [rows](src/viewer/rows.ts) |
| Setup proves only configuration presence | Still open. Installation indicators do not perform a connection test or verify publication from a real session. | [integration UI](src/integrations/AgentIntegrations.svelte), [config checks](src-tauri/src/integrations/config.rs) |
| Notes accumulate without cleanup | Partly addressed by dismissal and optional auto-clear. There is no undo or recoverable archive. | [dismissal](src-tauri/src/desktop/commands.rs), [pruning](src-tauri/src/status/domain.rs) |
| Documentation describes obsolete source menus | README now reflects the structured product and upgrade requirements. Its claim of an OS-held lock does not match the current existence-file implementation. | [README](README.md), [store lock](src-tauri/src/status/store.rs) |

## Users and core journeys

The code and skill establish an intended workflow, not a measured customer segment. All personas and usage-frequency assumptions below remain hypotheses.

### 1. Connect an agent or upgrade an installation

**User/trigger:** a developer installs the app, changes harness, moves the executable, or upgrades from the Markdown/version-1 model.

**Today:** open Settings, install or update the skill, register MCP separately, restart old processes as directed by the README, and hope a new chat publishes. The bundled skill requires an actual harness session ID and tells the agent to skip reporting if none is available.

**Desired outcome:** one supported harness publishes a visible test point, with clear recovery when session identity, configuration, executable path, or schema compatibility prevents it.

**Observed friction:** separate skill/MCP lists, no verification action, and no first-run guidance in an empty viewer. **Code-derived risk:** updating a skill removes the existing target directory before copying; a failed copy can leave an incomplete installation or remove local customization. A registration row can appear configured despite a bad executable command.

Sources: [integration UI](src/integrations/AgentIntegrations.svelte), [skills installer](src-tauri/src/integrations/skills.rs), [registration check](src-tauri/src/integrations/config.rs), [bundled skill](src-tauri/resources/skills/ctx-ppteer/SKILL.md), [upgrade notes](README.md).

### 2. Check progress while working elsewhere

**User/trigger:** a developer returns to the small viewer to decide whether an agent needs attention.

**Today:** scan project/branch groups and plain points, recognize DONE, and hover or focus a point for its age. Project sorting uses the latest record update; point age uses submission time. These are different clocks.

**Desired outcome:** quickly understand the latest reported work and whether the display is reliable.

**Observed friction:** blank waiting state; cached reports look unchanged after a waiting snapshot; session attribution is absent; age controls are hidden until hover/focus. **Code-derived friction:** the health enum and source modification time do not drive the UI, and error text sits at the bottom of the list. Point submission age does not indicate agent liveness or last meaningful edit.

Sources: [viewer](src/viewer/Viewer.svelte), [controller](src/viewer/controller.ts), [rows](src/viewer/rows.ts), [snapshot](src-tauri/src/status/snapshot.rs).

### 3. Clear clutter without losing important work

**User/trigger:** a developer dismisses an irrelevant point or enables retention.

**Today:** dismiss one point with a stable ID and entry timestamp guard. Failed dismissal restores the row. Auto-clear is disabled by default; when enabled, it removes all points older than their submission cutoff while the app is open. Shortening retention requires an Apply action.

**Desired outcome:** keep the board useful while understanding what disappears and how to recover a mistake.

**Observed behavior:** a newly edited unfinished point can still expire because its submission timestamp stays old. This matches the documented policy and is not a timestamp regression. **Code-derived gaps:** there is no undo, removal preview, completion-aware retention, or archive. The UI falls back to entry time when a legacy point lacks submission time, but pruning retains such points. Saving retention can succeed before pruning fails, producing a partial-success state.

Sources: [controller](src/viewer/controller.ts), [retention UI](src/settings/RetentionSettings.svelte), [domain](src-tauri/src/status/domain.rs), [retention command](src-tauri/src/desktop/commands.rs).

### 4. Monitor multiple projects and sessions

**User/trigger:** a developer has several projects, branches, or conversations active.

**Today:** independent sessions have separate records; the viewer combines all points with the same folder and branch. Sort orders help find recently written projects. Animated group removal and reordering preserve visual continuity.

**Desired outcome:** identify the relevant work without confusing its source or scanning every point.

**Observed friction:** two fixture sessions on one project/branch appear as one unlabeled group. **Hypothesis:** this causes confusion when reports disagree or two checkouts share a basename. Some users may prefer aggregation, so exposing raw session IDs everywhere would be premature.

Sources: [domain identity](src-tauri/src/status/domain.rs), [row grouping and sorting](src/viewer/rows.ts), [motion fixture](tests/viewer-motion.js).

### 5. Resume after a crash or broken data

**User/trigger:** an MCP process stops abruptly, a saved document becomes invalid, or the watcher fails.

**Today:** malformed JSON is preserved, the viewer can retain the last valid document, and the watcher retries with periodic reconciliation. However, write locking creates a sidecar file with exclusive creation and removes it only on normal Drop. It is not an OS-held advisory lock. A leftover file makes future writes/dismissals/pruning wait indefinitely. Startup pruning can also block startup when retention is enabled.

**Desired outcome:** resume reporting without manual lock deletion, silent data loss, or an application that appears frozen.

**Observed/code-derived distinction:** the synthetic-lock stall and recovery were reproduced in a temporary directory. A killed-owner scenario was not run against live app data; its consequence follows from the unbounded loop and Drop-only cleanup.

Sources: [store lock](src-tauri/src/status/store.rs), [startup](src-tauri/src/lib.rs), [watcher recovery](src-tauri/src/status/watcher.rs).

## Product vision

Help developers supervising coding agents keep an accurate, compact view of reported work across local projects, recognize what needs attention, and clear old reports with confidence.

This builds on structured points, completion flags, session isolation, sorting, dismissal, and the small persistent viewer. The product is becoming a personal attention board. Its next gains should come from dependable reporting and understandable state rather than more decorative motion.

### Principles

- Make unavailable, empty, and cached states distinguishable.
- Preserve independent reports and recover from interrupted writes.
- Verify setup through a real publication, not config presence alone.
- Keep routine viewing quiet and compact; expose attribution on demand.
- Separate submission age, last report time, completion, and agent liveness.
- Make cleanup deliberate and recoverable.
- Keep status local; evolve the structured contract with clear compatibility rules.

### Non-goals

- Running, scheduling, or controlling agents.
- Turning status points into assignments, sprints, or a task-management system.
- Restoring arbitrary Markdown viewing or whole-document editing.
- Cloud synchronization, team accounts, or remote collaboration.
- Full transcript/history retention.
- More harness adapters or animation options without evidence of user benefit.

## Prioritization

Use `value × strategic fit × confidence ÷ effort`.

Value and fit range from 1 to 5. Confidence is a fractional judgment about evidence and expected benefit, not a statistical estimate. Effort is relative scope: 1 is a contained change, 2 spans a flow or native/frontend boundary, and 3 changes persistence or the public contract. Estimates are not delivery dates.

Scores below are recomputed from explicit inputs and supersede the earlier audit's scores. Equal scores are ordered by failure severity. Dependencies gate implementation even when a dependent item has a higher score. Success thresholds are proposed pilot targets, not claims about measured usage.

| Rank | Phase | Update | Value | Fit | Confidence | Effort | Score |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Now | Recover from interrupted writes | 5 | 5 | 95% | 2 | 11.88 |
| 2 | Now | Explain empty, cached, and unhealthy states | 5 | 5 | 95% | 2 | 11.88 |
| 3 | Now | Verify first publication and upgrades | 4 | 5 | 85% | 2 | 8.50 |
| 4 | Next | Make dismissal and retention recoverable | 4 | 5 | 85% | 3 | 5.67 |
| 5 | Next | Clarify project and session attribution | 4 | 4 | 75% | 3 | 4.00 |
| 6 | Next | Focus the board on relevant reports | 4 | 4 | 65% | 3 | 3.47 |
| 7 | Later | Opt-in alerts for input-needed transitions | 3 | 4 | 40% | 3 | 1.60 |

## Now

### 1. Recover from interrupted writes

**Problem/evidence:** an abandoned existence lock can block all mutations indefinitely, including startup pruning. The synthetic-lock probe reproduced waiting and subsequent recovery. See [store.rs](src-tauri/src/status/store.rs), `lock` and `SidecarLock::drop`; [startup](src-tauri/src/lib.rs).

**User story:** As a developer resuming agent work after a crash, I want reporting to recover automatically or fail with an actionable message, so that my status board stays usable.

**Change/vision fit:** use crash-releasing interprocess coordination with bounded waiting and useful contention errors. Preserve atomic replacement. Keep blocking work from freezing MCP/native async request handling. Correct the lock description in the README. This is technical work required for reliable reporting.

**Impact/effort/confidence:** high impact when triggered; medium effort; 95% confidence in the failure mechanism, unknown incident frequency.

**Dependencies:** platform-safe locking, consistent use by write/dismiss/prune, and a non-destructive recovery policy. Never remove a lock just because it is old while another process might own it.

**Acceptance:** terminate a lock owner in a disposable store and verify a new writer succeeds; live contention ends with success or a bounded error; concurrent distinct-session writes remain intact; retention-enabled startup remains responsive; a failed write leaves valid bytes unchanged.

**Success:** zero indefinite waits in the crash/contention matrix on macOS, Windows, and Linux; uncontended and recovery operations complete or produce an explicit error within a proposed five-second ceiling.

### 2. Explain empty, cached, and unhealthy states

**Problem/evidence:** a waiting snapshot can render a blank board or retain reports without a warning. Existing errors render below all content. See [snapshot](src-tauri/src/status/snapshot.rs), `snapshot_for`; [controller](src/viewer/controller.ts), `commitPending`; [viewer](src/viewer/Viewer.svelte).

**User story:** As a developer checking progress, I want to know whether there are no reports or whether reporting is unavailable, so that I do not trust an old board or troubleshoot a healthy empty one.

**Change/vision fit:** add distinct first-run, empty-after-cleanup, waiting, and cached-error states. Keep a compact health message visible above scrolling content, with retry and setup/diagnostic guidance. Show last successful read/report time where useful. State that a readable document does not prove an agent is active.

**Impact/effort/confidence:** high across setup and return visits; medium effort; 95% confidence in observed ambiguity.

**Dependencies:** controller must retain health and last-success metadata independently of row animation/selection. Do not delay critical errors until a selection clears. Use #1's bounded errors.

**Acceptance:** first launch, last-point dismissal, expired board, deleted file, invalid JSON, permission failure, and watcher failure are distinguishable. Cached reports remain labeled. Recovery clears the warning. Keyboard and screen-reader users can reach guidance in a compact window.

**Success:** 90% of five or more pilot users identify the correct state and next action within five seconds. Every failure fixture displays a visible message without scrolling.

### 3. Verify first publication and upgrades

**Problem/evidence:** skill installation, MCP configuration, schema compatibility, and real session identity are separate requirements. Current registration checks presence and Settings has no verify action. Updating a skill removes the destination before copying. See [integration UI](src/integrations/AgentIntegrations.svelte), [config](src-tauri/src/integrations/config.rs), [skills](src-tauri/src/integrations/skills.rs), [README](README.md), [bundled skill](src-tauri/resources/skills/ctx-ppteer/SKILL.md).

**User story:** As a new or upgrading user, I want to prove my chosen harness can publish a status using its real session ID, so that I know reporting is ready.

**Change/vision fit:** provide a per-harness checklist with separate configured, reachable, compatible, and publication-verified states. Include a copyable fresh-session test prompt, restart guidance, and an explicit unsupported-session-ID outcome. Make skill replacement staged and recoverable. Explain intentional skill/MCP support asymmetries.

**Impact/effort/confidence:** high activation and upgrade benefit; medium effort; 85% confidence, pending real first-use sessions.

**Dependencies:** bounded MCP calls from #1 and empty-state guidance from #2; installed executable and skill version checks; test publication explicitly initiated by the user.

**Acceptance:** a clean profile publishes a real point with a supported harness; a moved executable, old skill, old schema reader, malformed config, and missing session ID produce specific guidance. A failed skill copy preserves the previous installation. Unrelated harness config survives.

**Success:** at least 80% of pilot users publish within five minutes without assistance; no failed verification is labeled ready; installed skill update failure leaves a usable previous version.

## Next

### 4. Make dismissal and retention recoverable

**Problem/evidence:** guarded dismissal is implemented, but successful removal has no undo. Auto-clear deletes unfinished and completed points by original submission time; recent edits can expire. Legacy missing timestamps have inconsistent displayed versus effective expiry. Saving retention before prune failure can leave settings applied despite an error. See [domain](src-tauri/src/status/domain.rs), [retention command](src-tauri/src/desktop/commands.rs), [retention UI](src/settings/RetentionSettings.svelte).

**User story:** As a developer clearing old reports, I want to preview removals and recover mistakes, so that I can keep the board concise without losing active context.

**Change/vision fit:** add short-lived undo and a bounded local recently-cleared list for dismissal/expiry, not a transcript history. Preview affected counts before enabling or shortening retention. Clarify submission-age expiry and decide with users whether completed-only or last-meaningful-edit retention better matches intent. Report settings-applied/prune-failed distinctly.

**Impact/effort/confidence:** high for users who enable cleanup; large effort; 85% confidence in mechanics, lower confidence about the best policy. Auto-clear is off by default, which keeps this below universal setup/health work.

**Dependencies:** #1, stable point IDs already present, bounded recovery storage, and an explicit legacy timestamp rule. Do not restore over newer agent reports or blindly reuse unknown IDs.

**Acceptance:** undo a dismissal without duplicating a point or restoring superseded text; preview matches actual removals; recently edited unfinished and missing-timestamp fixtures follow the selected policy; failed pruning explains the saved policy; restored reports obey clear expiry rules.

**Success:** 90% of seeded cleanup mistakes are recoverable within 30 seconds; at least 90% of pilot users predict which points expire; recovery storage stays within a documented bound.

### 5. Clarify project and session attribution

**Problem/evidence:** project/session storage isolation is real, but the UI merges same-folder/same-branch records without provenance. Same-basename checkouts can appear to be one project. See [domain](src-tauri/src/status/domain.rs), `upsert`; [rows](src/viewer/rows.ts), `viewerRows`.

**User story:** As a developer with overlapping conversations or checkouts, I want to identify the source of a report, so that I can distinguish contradictory or unrelated work.

**Change/vision fit:** validate a stable project identifier separate from display name, then add optional readable session/harness attribution on demand. Preserve the compact aggregate view. Do not expose raw session IDs as the default experience.

**Impact/effort/confidence:** high for overlapping work, uncertain prevalence; large effort because identity changes the contract; 75% confidence.

**Dependencies:** decide whether identity means repository or checkout; backwards-compatible schema/MCP/skill evolution; a legacy display policy. Session IDs remain identifiers, not authentication.

**Acceptance:** two same-name checkouts can remain visually distinct, two sessions can be attributed without showing raw IDs, old clients still publish under a documented fallback, and migration never duplicates or merges records silently.

**Success:** 90% correct source identification in a pilot containing duplicate names and conflicting session reports; zero identity collisions in the supported migration matrix.

### 6. Focus the board on relevant reports

**Problem/evidence:** sorting helps, but all points remain in one list, with no filter or user-controlled collapse. Group collapse currently refers to removal animation, not a navigation feature. Hidden age controls and retained completed reports add scanning work. See [viewer](src/viewer/Viewer.svelte), [rows](src/viewer/rows.ts).

**User story:** As a developer supervising several projects, I want to filter and temporarily hide irrelevant reports, so that I can find work needing attention quickly.

**Change/vision fit:** add a minimal project jump/filter, optional unfinished-only view, and user-controlled group collapse. Preserve filters, selection, and reading position across updates. Define whether recency means latest write or meaningful content change before expanding sorting.

**Impact/effort/confidence:** medium to high if multi-project volume is common; large effort across view state and animation; 65% confidence until workload observation.

**Dependencies:** attribution decisions in #5 for duplicate names; stable IDs and sorting already exist. Filtering must not delete records or accidentally broaden MCP read access.

**Acceptance:** in a ten-project/50-point fixture, find a named project, inspect unfinished work, and return to all reports; background updates preserve position and collapsed state; keyboard navigation and reduced motion work.

**Success:** halve median find-the-relevant-report time against the current viewer without increasing wrong-project choices.

## Later

### 7. Opt-in alerts for input-needed transitions

**Problem/evidence:** DONE is structured, but blocked/input-needed state is free text and there are no native notifications. Missing an important transition is a hypothesis, not a demonstrated customer problem. See [Point](src-tauri/src/status/domain.rs), [MCP](src-tauri/src/mcp.rs), [viewer](src/viewer/Viewer.svelte).

**User story:** As a developer leaving agents running, I want an optional alert when a project needs my input, so that work does not wait for my next glance.

**Change/vision fit:** only after validation, introduce a backwards-compatible input-needed state and per-project opt-in alerts. Deduplicate by stable identity/transition; support mute and quiet periods. Keep ordinary progress silent.

**Impact/effort/confidence:** potentially useful, with interruption and false-positive costs; large effort; 40% confidence.

**Dependencies:** #2 and #5; agreed transition semantics, native permissions, and restart-safe deduplication. A missing report must not be inferred to mean blocked or complete.

**Acceptance:** one alert per confirmed transition, none for polling/restart or unrelated text edits, clear mute controls, and navigation to the correct project.

**Success:** halve response time to genuine input-needed events while fewer than 10% of pilot alerts are judged unnecessary. Do not build this if observed workflows do not need it.

## Starting point and tradeoffs

Start with #1, #2, and #3. Crash recovery protects the core write path. Clear health/empty states prevent misplaced trust and make setup understandable. Verified publication converts installation into a working user outcome. These directly support the existing product without requiring a new persona or workflow.

Normal write serialization and the motion queue do not need a new broad refactor. Focus technical work on the failure paths that block user outcomes. Retention recovery comes before full history; it addresses a new irreversible interaction already present. Project filtering follows source attribution so a faster interface does not make ambiguity easier to miss. More animation and color controls have lower value than these gaps.

## Technical prerequisites and maintenance

- Product prerequisites: crash-safe bounded coordination for reliable publishing; health state propagation for trustworthy reading; staged skill installation for safe upgrades; bounded recovery storage for undo; identity migration for attribution.
- Routine maintenance: retain the current regression suites and consider running them before release packaging. The supplied release workflow builds bundles but does not explicitly run the controller, Rust, or motion suites. Native smoke checks should cover fresh install, upgrade, link opening, startup with retention, and crash recovery.
- Measure before optimizing: pruning currently rewrites the document even when nothing expires. This can generate unnecessary file events, but no measured user-visible performance problem was established, so it is not a separate roadmap feature.

Sources: [release workflow](.github/workflows/release.yml), [store pruning](src-tauri/src/status/store.rs), [watcher](src-tauri/src/status/watcher.rs).

## Validation that could change priorities

1. Observe five developers across a single-project and a multi-project session. Record project/session counts, glance frequency, and how they choose their next action.
2. Test clean installation and upgrade with the most-used harnesses. Separate configuration, handshake, real session identity, and actual publication failures.
3. Reproduce owner-process termination and contention on all supported operating systems. This is a release gate for #1, not a request to expand platform scope.
4. Ask users what dismissal means: permanently discard, hide locally, or acknowledge. Clarify whether the publishing agent may later reintroduce the report.
5. Test expiry expectations with unfinished, recently edited, completed, caller-timestamped, and legacy points. If cleanup confusion is frequent, move #4 into Now.
6. Determine whether duplicate checkout names and simultaneous conversations occur often enough to justify a new project identity. Keep existing aggregation if it matches observed needs.
7. Clarify whether the board reports submitted work, current agent liveness, or actionable blockers. The current data proves only submitted reports.
8. Establish local or consented measurement baselines for time to first status, state comprehension, find time, cleanup recovery, and missed input requests. Proposed targets are decision aids, not production telemetry claims.
