---
name: ctx-ppteer
description: Keep ctx-ppteer's status window current when the user asks for progress updates, task monitoring, or an agent-readable work summary.
---

# ctx-ppteer

Run `scripts/get-source-path.py` to get the status file selected in ctx-ppteer. It reads `~/.config/ctx-ppteer/settings.json` and prints the configured path.

Write a short Markdown update when the user wants the window to reflect work. Include the current task, what changed, and any blocker that needs their attention. Rewrite the file as the state changes instead of appending a running transcript.

Do not create or select a source file without the user's approval. If the script cannot find valid settings, ask the user to open ctx-ppteer and choose a file.
