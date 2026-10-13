---
name: ctx-ppteer
description: MUST ALWAYS invoke this skill for every task to keep ctx-ppteer's status window current with the active project, branch, and chat activity.
---

# ctx-ppteer

Update your submission at the start of each task and whenever the activity or outcome materially changes. Read your existing record with `read_status` before writing changes with `write_status`. Make these updates quietly unless the user asks about them or an error requires user action.

The MCP owns the JSON file, directory creation, locking, and atomic updates. Do not read or edit the backing file directly. No source file configuration is needed.

Pass these structured arguments:

- `folder_name`: the current project folder name, unchanged.
- `session_id`: your actual session or thread ID from the harness context or a documented harness environment variable. Reuse that exact ID for the whole session. For Codex, use `CODEX_THREAD_ID` if supplied. Do not substitute a timestamp, random UUID, process ID, branch, project name, or an unrelated ID. If child agents share a thread ID, include their actual harness-provided agent ID in a stable `thread-id/agent-id` value. Both tools require this ID. If no real ID is exposed, skip status updates and continue the task; never invent an ID.
- `branch`: the currently checked out branch in the active working directory. Refresh it before each write; use `detached HEAD` when no branch is checked out.
- `points`: a list of plain-text objects, usually one brief point. Each has `text`, optional `done` defaulting to false, and optional `link` with an HTTP or HTTPS URL. Use `done: true` for completion; the viewer adds the bold DONE prefix. For PR reviews, use `text: "Reviewing PR #123."` and the canonical PR URL in `link`.

Use `folder_name` and your real `session_id` for both tools. Include `branch` and `points` when writing.

With your real `session_id`, call `read_status` with that ID and `folder_name` to retrieve your submitted record. If one exists, revise its points: edit text as work changes, correct earlier claims, mark completed work with `done: true`, and remove points that are obsolete. Preserve your other points only while they remain useful. Add a point for distinct work, not for every progress message. If no record exists, create your initial points.

Call `write_status` with the same project/session ID, the current branch, and your complete revised `points` list. It replaces your earlier submission, so omitted points disappear. For example, change "Investigating concurrent writes." to "Concurrent writes fixed and verified." in the existing point and mark it done rather than retaining both versions. Do not resend an unchanged record.

Keep this as an editable account of your current work, not a message history.

Keep text brief. Do not send Markdown syntax, commit hashes, or other revision identifiers in point text. Mention blockers only when critical and unresolved.
