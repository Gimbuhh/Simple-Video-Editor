# Releasing

Version 1.0.0 is the initial release, dated 2026-10-09. The private repository is [Gimbuhh/simple-video-editor](https://github.com/Gimbuhh/simple-video-editor). Source versioning uses the `<Version>` in `Development/src/SimpleVideoEditor/SimpleVideoEditor.csproj`; version 1.0.2 corresponds to tag `v1.0.2`.

## Initial repository upload

The initial upload uses a verified local portable ZIP to bootstrap dependency recovery: push source, attach the ZIP and checksum to a draft `v1.0.0` release targeting that source commit, run Windows checks, and publish after verification. Upstream libmpv uses a rolling release that removes old assets; the retained ZIP serves as the durable dependency mirror without storing native binaries in Git. Its release body comes from `release-notes/1.0.0.md`.

## Subsequent releases

`MediaMirrorTag` in the dependency manifest identifies a release whose ZIP contains those exact pinned native files. Keep that release available, including drafts. When native pins change, preserve a matching new bundle and update the mirror tag; ordinary source-only version changes can keep the existing mirror tag. Setup verifies all restored binaries against the manifest and rejects mismatched bundles.

1. Update the application version, `CHANGELOG.md`, and `release-notes/<version>.md` together. Keep the README's version and scope current. Record final user-visible outcomes, rather than intermediate development iterations.
2. Run setup, repository checks, core tests, desktop UI checks, and a portable build. Check NVIDIA encoding locally when available. Launch the packaged executable and check Shadowplay alongside a running game when capture compatibility is affected.
3. Commit source and documentation on a feature branch, push that branch, and open a pull request targeting `main`. Keep generated bundles, personal recordings, projects, and verification output outside Git. Direct pushes to `main` are prohibited; enable the local push guard described in the development guide.
4. After PR checks pass and the PR is merged within the user's authorization, tag the merged `main` commit as `v<version>` and push that tag. The release workflow validates the tag, runs CPU/media checks, builds the portable ZIP and checksum, and creates a **draft release** with the committed release notes.
5. Download and check the draft assets, then publish when authorized and ready. The workflow does not publish automatically. If a release already exists for that tag, its assets and description remain untouched; the workflow still validates and builds the tagged source.

Draft assets are `SimpleVideoEditor-win-x64.zip` and `SimpleVideoEditor-win-x64.zip.sha256`. Extract the complete ZIP and open `Simple Video Editor.exe`; keep `Support` beside it. Dependency restoration accepts both the original flat ZIP layout and the Support layout introduced in 1.0.2. The `Windows checks` workflow also keeps successful build artifacts briefly for review without creating a release.

Local build scripts do not create GitHub repositories or releases. Repository creation, tag pushes, and publication are separate actions.
