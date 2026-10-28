# C# 1.0.4 and Rust 1.1.0 — 2026-10-28

Rust 1.1 adds HDR preview/export, per-section audio and framing, HEVC, timeline filmstrips and decoded-timestamp VFR navigation. The graphite interface and functional source cutover are complete; final physical acceptance remains in the [release checklist](release-readiness.md).

## Measured artifacts

The preserved C# source is `dd65df13a41490e8168a624b65fcff6a27956940`; its executable SHA-256 is `703eb67be5b6ff6035ac06de2f0a7dec06685299643e7d5ab26dc4e46e129cf6`. The measured Rust executable SHA-256 is `1e3599a09e3537098ffcfbe097d05d549496e754a427be90f46459e30dfc05b0`. Their embedded versions are 1.0.4.0 and 1.1.0.0, and every native media binary matches by hash.

The extracted portable bundles are approximately 469 MB for C# and 369 MB for Rust. Downloads are about 199 MB and 150 MB respectively: roughly 25% less download and 21% less extracted storage. Rust has no bundled .NET runtime. Both versions are portable and need no separate .NET installation.

## Visible resource measurements

| Paused default-window case | CPU, % of one logical core | Working set, MiB | Private commit, MiB |
| --- | ---: | ---: | ---: |
| C# empty project | 0.09 | 120.4 | 128.7 |
| C# generated 4K preview | 0.00 | 674.1 | 678.6 |
| Rust empty project | 0.00 | 160.9 | 268.2 |
| Rust native 4K preview | 0.09 | 457.5 | 728.2 |
| Rust software 4K preview | 0.17 | 726.7 | 852.7 |

Each value is the mean of three visible, non-minimized launches. Each helper settles for ten seconds, samples for six seconds and closes normally. Generated 4K SDR media and the same 0.5–2.5 second retained project interval open through ordinary filename startup. Source and executable hashes are preserved. The host is a Ryzen 7 9800X3D with 16 logical processors and an RTX 5070 Ti. Default C#/Rust client sizes and preview backends differ; this is a bounded default-app idle comparison with uncontrolled OS cache/system load.

CPU readings are near the sampling floor; zero records no increment during that interval. They do not establish a general speed advantage. Process working set and private commit measure different resources and exclude dedicated GPU memory, child processes and export throughput.

Native preview uses less process working set in this generated case. Empty and software-preview Rust cases use more than C#, and Rust private commit is higher in all cases. The rewrite therefore does not establish a universal memory-use win. Earlier idle-polling and seeking improvements remain documented with their original artifacts in the [comparison history](https://github.com/Gimbuhh/Simple-Video-Editor/blob/3817dd3002c495f64f92f18c7d0b6ef94d64ce4c/docs/rust-csharp-comparison.md).

## Verified behavior and tradeoffs

Rust restores project/media filename startup, ordered relative/Unicode paths, pointer-position timeline drops and linked undoable edits. Idle previews sleep, processing follows display-sized tiers, continuous scrubbing presents completed frames, and release settles on the exact source timestamp. Preview audio uses the default Windows output and can recover an unavailable endpoint without resetting video playback. Starting/opening projects releases previous preview resources.

The [status ledger](rust-rewrite-status.md) records current core, desktop, audio recovery/fallback, media/GPU, layout, physical monitor, startup and portable-package checks. Current-display HDR appearance is user-confirmed; the separate Windows/NVIDIA game-switch recording problem is user-confirmed as external. Broader physical HDR-toggle, mixed-DPI and full screen-reader workflows remain acceptance work.

Projects saved by 1.1 use schema 2 and cannot be read by 1.0.x. Keep original schema-1 projects and the published C# ZIP for rollback. The original ZIP hash remains `5503f27981238c9d38e9e0b31fce8075b0c26511727fdfd16f6c0ef772e86493`. No C# production runtime/source remains in the Rust app; the pinned historical oracle is retained only for isolated parity tests.

Evidence: version-comparison/b7c46c74995e41e5909d73231255fe97. Reproduce with `Development/scripts/compare-implementations.ps1 -LegacyApp <extracted-CSharp-bundle> -RustApp App -Source <generated-4K-SDR-recording> -Visible -OrdinaryStartup`. Temporary extracted bundles can be removed after measurement; reports, the original ZIP and Git history remain available.
