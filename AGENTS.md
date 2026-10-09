# Working on Simple Video Editor

This folder is the canonical Git checkout for the private repository:
`https://github.com/Gimbuhh/simple-video-editor` (`origin`, default branch `main`).

## Git workflow

- Use this repository for all future work. Inspect status and remotes before editing; preserve unrelated user changes.
- Use meaningful commits for completed requested changes. Push or publish within the user's authorization for that task; do not force-push, rewrite published tags, or replace release assets silently.
- Use Git history instead of new source-backup ZIPs or version-folder copies. Existing ignored local backups can remain.
- Commit source, tests, scripts, authored assets, and documentation. Keep App bundles, release ZIPs, SDK/native downloads, bin/obj, personal recordings, project/recovery files, shortcuts, and verification output ignored.

## Build and verification

- Run scripts from the repository root; see `docs/development.md`.
- `Development/src/SimpleVideoEditor/SimpleVideoEditor.csproj` is the authoritative app version. Keep README, changelog, release notes, and release metadata consistent.
- Run repository checks and core/media checks for relevant changes. Run desktop UI checks for interaction/layout changes; GPU tests require compatible NVIDIA hardware. Report any unavailable checks accurately.
- Preserve original recordings, saved trim boundaries, timeline gaps, and linked audio/video behavior.
- Build the portable app with `Development/scripts/build.ps1 -Zip`. Do not replace a running app or stop the user's editor; use `-OutputName App-Updated` if needed.
- After verifying a complete App bundle, `Development/scripts/clean.ps1 -BuildTools` removes reproducible build tools without removing the app or source.

## Releases

- Follow `docs/releasing.md`; keep native dependency pins and license notices together.
- Keep the release named by `MediaMirrorTag` available: its portable ZIP restores the exact native dependencies if upstream rolling downloads disappear.
- Future tag workflows prepare draft releases. Publish only with user authorization and after verification.
