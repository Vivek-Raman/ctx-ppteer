---
name: ctx-ppteer
description: MUST ALWAYS invoke this skill for every task to keep ctx-ppteer's status window current with the active project, branch, and chat activity.
---

# ctx-ppteer

You MUST ALWAYS invoke this skill for every task. Call `write_status` at the start of work and whenever the activity, outcome, or blocker materially changes.

Treat these updates as routine background coordination: make them quietly and seamlessly, without announcing, narrating, or calling attention to the skill or its invocation in user-facing commentary or final responses. Mention them only when the user explicitly asks about ctx-ppteer or its status updates, or when its configuration/error requires user action.

Pass the current project folder name as `folder_name`. The server uses it unchanged as the H2 and preserves status sections for other projects. Pass `status` as short Markdown in this form:

```md
branch-name

- Current activity.
- Relevant progress, result, or blocker.
```

Keep the branch as ordinary text, not a heading or code formatting. When reviewing a pull request, include the PR number and commit hash, and use a canonical Markdown PR link: `- Reviewing PR [#123](https://github.com/OWNER/REPOSITORY/pull/123) at \`a1b2c3d\`.`

Branches can change during a thread (including through another harness or worktree action). When preparing a `write_status` call, use the branch currently checked out in the active working directory rather than assuming an earlier branch is still current. Treat this as a quick refresh that is part of the status write, not as a separate monitoring or investigation task.

Use `read_status` with the project folder name to retrieve the current section before a change when that context is useful.

Do not create or select a source file without the user's approval. If the tool reports missing settings, ask the user to open ctx-ppteer and choose a file.
