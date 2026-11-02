# 1.1.0 release checklist — 2026-11-03

Version 1.1.0 introduces the Rust implementation and the editing features described in the changelog and release notes. This checklist records verification and compatibility coverage for the release. PR #8 tracks the source changes; merging and publishing follow the [release procedure](releasing.md).

## Software verification

The runtime built from `1c4b736` passes core checks, all 14 NVIDIA/media checks and 350 desktop assertions. Additional native/software UI-scale runs pass 79 assertions and 40 layouts at 100–200%. Indoor/outdoor Fortnite comparisons and a 24-seek production preview benchmark also pass. Windows CI passes on that source revision. Later revisions require their own passing CI before merge. Earlier broader recovery and startup results retain their recorded executable hashes. Results and exact executable hashes are recorded in the [status ledger](rust-rewrite-status.md); generated recordings and full reports remain in ignored artifacts.

| Gate | Scope |
| --- | --- |
| Core and persistence | Formatting, strict Clippy, workspace tests, compatibility readers, exact trims/gaps, atomic save/recovery, undo/redo and release inspector. |
| Native media and NVIDIA exports | SDR/PQ/HLG, scene-aware and CPU color conversion, geometry, exact/VFR stepping, mixed audio, gain/fades, black audio tails, cancellation and unchanged sources. |
| Desktop interaction | Native/software preview, audible selected-track playback, timeline drops, cursor/trim behavior, relinking and audio-tail scrubbing. Rendered layouts cover 100–200% app scale. |
| Failure recovery | Missing audio endpoint, native-to-software fallback and release of old players/caches on project reset. |
| Startup and portable package | Repeated isolated startup, ordinary filename opening, Unicode relocation, version/checksum/native pins/licenses, no .NET runtime and nine packaging safeguards. |
| Windows CI | The final feature-branch revision must pass before merge. Local passing checks do not substitute for that revision's CI result. |

The [C#/Rust comparison](rust-csharp-comparison.md) records measured benefits and memory tradeoffs. The rewrite uses a smaller portable package and hardware preview where supported; it does not promise lower memory use in every workflow.

## Physical acceptance

The user confirms HDR appearance on the current display and improved HDR-to-SDR conversion. The separate Windows/NVIDIA game-switch recording issue is confirmed external to this project. Earlier automated native/software HDR/SDR monitor round trips passed without changing Windows settings; that setup used two 96-DPI monitors. These results do not cover mixed-DPI transitions.

Manual Windows HDR toggles, genuine mixed-DPI transitions and a complete screen-reader workflow remain unverified. App scaling, framebuffer comparisons and accessibility-tree checks establish narrower contracts. These are outstanding acceptance checks, not confirmed defects or certified configurations.

NVIDIA can separately recognize the editor as a game. A [per-editor driver exclusion](nvidia-capture.md) resolves that classification on the tested machine without changing game/capture settings. It is optional machine configuration using an undocumented vendor flag, not a portable app-wide guarantee. Isolated SDR/HDR/software launches and profile rollback pass; game/editor transitions with active replay still require a matched live test.

## Merge, release and rollback

- Finish testing the packaged App and explicitly approve the PR merge after its checks pass. PR #8 remains open until then.
- Create a version tag only from the merged main commit. The workflow prepares a draft; publishing requires separate authorization.
- Preserve published 1.0.x releases, the pinned native-dependency mirror and original projects. Projects saved by 1.1 cannot be opened by 1.0.x.
- Sections join with hard cuts. Transitions, music-file import and additional audio mixing lanes remain later work.
