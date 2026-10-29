# 1.1.0 release checklist — 2026-10-29

The Rust rewrite and graphite interface are implemented. Version 1.1.0 remains unreleased. The user confirms that the installed HDR-to-SDR correction looks much better. Pixel comparisons and the tiny-highlight regression cover preview, thumbnails and exports. Windows CI passes on the HDR correction commit `a71af30`; later changes require their own checks. The standard CPU fallback remains separately verified. PR #8 stays open until the user finishes testing the installed candidate and explicitly approves merging.

## Software acceptance

- Core contracts, strict formatting/Clippy, legacy project compatibility and source protection.
- Native/software preview, exact/VFR trims, timeline drops, undo/redo, thumbnails and keyboard focus.
- Independent audio separation, per-item volume, trims/splits/moves, atomic undo/save/reopen and offset/mixed/tail exports.
- Audible two-track playback, unavailable-endpoint recovery and native-to-software fallback.
- Named playback controls and labelled trim fields in the accessibility tree.
- Project reset releases old players and preview caches; startup stress closes isolated helpers normally.
- SDR/PQ/HLG and geometry exports, NVIDIA encoding, cancellation and immutable source hashes.
- Layouts at 100–200% app scale; physical HDR/SDR monitor round trips with native bounds and transfer checks.
- Portable ZIP version, checksum, native pins/licenses, Unicode relocation and ordinary filename startup.
- Current Windows CI passes on the feature-branch commit before any merge.

Detailed final results and exact executable hashes are recorded in the [rewrite status](rust-rewrite-status.md). The [resource comparison](rust-csharp-comparison.md) preserves measurement scope and tradeoffs. Generated fixtures and full reports stay in ignored local artifacts.

## Final physical acceptance

The user has confirmed HDR appearance on the current display and that the separate Windows/NVIDIA game-switch recording problem is external. Connected-monitor transitions pass without changing Windows settings. Both connected monitors currently use 96 DPI.

Manual Windows HDR toggles, actual mixed-DPI monitor transitions and a full screen-reader workflow remain unverified. Simulated app scaling and accessibility-tree inspection cover narrower contracts. These checks require the corresponding setup and user observation before final release approval; they are not reported as fixed defects or certified combinations.

## Release and rollback

Review and test the packaged App, then approve the PR merge after CI passes. Tag only the merged main commit; the release workflow creates a draft. Publishing still requires explicit authorization. Preserve published 1.0.x assets and original schema-1 projects: projects saved by 1.1 cannot be opened in 1.0.x. Sections continue to use hard cuts; transitions remain later work.
