---
name: ctx-ppteer
description: Always keep ctx-ppteer's status window current with the active project, branch, and chat activity.
---

# ctx-ppteer

Invoke this skill for every task. Call `write_status` at the start of work and whenever the activity, outcome, or blocker materially changes.

Pass the current project folder name as `folder_name`. The server uses it unchanged as the H1 and preserves status sections for other projects. Pass `status` as short Markdown in this form:

```md
branch-name

- Current activity.
- Relevant progress, result, or blocker.
```

Keep the branch as ordinary text, not a heading or code formatting. When reviewing a pull request, include the PR number and commit hash, and use a canonical Markdown PR link: `- Reviewing PR [#123](https://github.com/OWNER/REPOSITORY/pull/123) at \`a1b2c3d\`.`

Use `read_status` with the project folder name to retrieve the current section before a change when that context is useful.

Do not create or select a source file without the user's approval. If the tool reports missing settings, ask the user to open ctx-ppteer and choose a file.
