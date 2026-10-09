---
name: release-ctx-ppteer
disable-model-invocation: true
description: Create and verify a major or minor GitHub release for ctx-ppteer, with an explicit version and publish confirmation.
---

# Release ctx-ppteer

Use this skill only when explicitly invoked to release this repository. Model invocation is disabled.

## Decide the version

1. Read the current version from `package.json`, `package-lock.json`, and `src-tauri/Cargo.toml`. They must agree before continuing.
2. The user must choose a **major** or **minor** SemVer bump. If their invocation does not clearly choose one, ask which bump they want and do nothing else that changes repository or GitHub state.
3. Compute the new version (`major`: `X.0.0`; `minor`: `X.Y.0`) and present both values, for example: `0.1.3 → 0.2.0`.
4. Wait for an unambiguous user go-ahead before editing files, committing, pushing, tagging, or creating a release. A confirmation from an earlier release attempt does not apply after the version or target commit changes.

## Preflight after approval

- Recheck the current branch, upstream, working tree, GitHub CLI authentication, and remote repository. Stop and report any issue that could release unrelated or uncommitted work.
- Release from the repository's default branch (`main`) only. Update it from `origin/main` without overwriting local work; if it has moved or diverged, resolve that before proceeding.
- Confirm the proposed `v<version>` tag and GitHub release do not already exist. Do not overwrite, retag, or delete an existing release or tag.

## Prepare and publish

1. Update the version consistently in:
   - `package.json`
   - `package-lock.json`
   - `src-tauri/Cargo.toml`
   - the root-package entry in `src-tauri/Cargo.lock`, if Cargo updates it

   Prefer `npm version <new-version> --no-git-tag-version` for the npm files. Update the Cargo manifest, then run a Cargo command that refreshes its lockfile when needed. Do not change dependency versions.
2. Inspect the diff and run `git diff --check`, `npm run build`, and `cargo check --manifest-path src-tauri/Cargo.toml`. Resolve release-related failures before committing.
3. Commit only the version changes with `Release v<new-version>`, then push that commit to `origin/main`.
4. Create the GitHub release with GitHub CLI, targeting the pushed commit and using tag `v<new-version>`. Use generated notes unless the user supplied release notes:

   ```sh
   gh release create "v<new-version>" --target "$(git rev-parse HEAD)" --title "ctx-ppteer v<new-version>" --generate-notes
   ```

## Follow through to completion

The release workflow is `.github/workflows/release.yml`; it runs when a release is created and builds macOS, Linux, and Windows bundles.

1. Find the workflow run associated with the new release, allowing briefly for it to appear, then follow it with `gh run watch <run-id> --exit-status`.
2. Treat the release as successful only when all workflow jobs have succeeded and `gh release view v<new-version> --json isDraft,url,assets` confirms a published (non-draft) release with uploaded assets. Report the release URL and run URL.
3. If the run fails, inspect `gh run view <run-id> --log-failed` and the failed job metadata, diagnose the cause, and fix issues that are clearly in scope.
   - For a transient infrastructure failure, rerun the failed jobs with `gh run rerun <run-id> --failed`, then watch and verify again.
   - For a source fix, commit and push the correction. The existing release tag still points to the old commit, so do not claim that new commit has fixed the current release.
   - Never force-move a tag or delete/replace a release. Explain the failure and ask the user for approval of a new release version and publication plan before making a corrective release.
4. Continue the check/fix/retry cycle until verified success or until a user decision is required. Do not report success from a release creation command alone.
