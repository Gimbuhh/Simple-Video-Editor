# C# 1.0.4 versus Rust 1.1.0

Compared on 2026-10-23. **Do not retire the C# version yet.** Rust is more capable for HDR and editing controls and has a smaller portable package, but the current candidate has baseline interaction regressions and higher measured empty-window idle costs. Changing language has not established an overall performance or reliability improvement. Visual refinement is a separate later pass; matching or improving on the C# interface is its goal, not a claimed current advantage.

## Exact comparison targets

| Artifact | C# | Rust |
| --- | --- | --- |
| Source | `dd65df13a41490e8168a624b65fcff6a27956940` | `c8deaff03c097a7c1af12e199eefa57a50eec9e1` |
| Embedded executable version | 1.0.4.0 | 1.1.0.0 |
| Executable SHA-256 | `703eb67be5b6ff6035ac06de2f0a7dec06685299643e7d5ab26dc4e46e129cf6` | `b6404bc435ff381f166dbfd8ed36cc72f2fce58b699881fef2333a9e8d28034a` |
| Executable bytes | 140,125,284 | 23,248,896 |
| Complete extracted bundle bytes, including documentation | 469,431,568 | 366,664,210 |
| Portable ZIP bytes | 199,337,788 | 148,863,934 |

Rust's complete bundle is 21.9% smaller and its ZIP is 25.3% smaller. Comparing only executable sizes would exaggerate the complete-app reduction. All media binaries in the compared Rust bundle match their C# counterparts by SHA-256. Both packages are portable; the C# distribution already bundles its runtime, so neither requires the user to install .NET separately.

The C# production source matches the source still present on `origin/main` at the comparison's start. Earlier cutover commits removed it from the Rust feature branch, not from main or history. The original release ZIP, local baseline/release bundles and isolated baseline source are preserved. No production implementation was changed to improve this comparison's result.

## Code and behavior findings

**Source-confirmed regressions in Rust:**

1. **Launch filenames are ignored.** C# handles a `.sveproject` filename or recording filenames in `Window_Loaded`. Rust's entry point handles internal verification switches, and its constructor handles the software-preview switch, but no production startup path opens/imports filename arguments. Launching with a project or dropping a file onto the executable therefore loses the old startup behavior. This does not imply that a Windows file association was registered by either app. References: [C# startup](https://github.com/Gimbuhh/Simple-Video-Editor/blob/dd65df13a41490e8168a624b65fcff6a27956940/Development/src/SimpleVideoEditor/MainWindow.xaml.cs#L163), [Rust entry point](https://github.com/Gimbuhh/Simple-Video-Editor/blob/c8deaff03c097a7c1af12e199eefa57a50eec9e1/Development/rust/crates/desktop-ui/src/main.rs).
2. **Timeline drops lose their placement.** C# derives the timeline time from the pointer and passes it through `InsertSourceClip`/`PlaceClip`. Rust's library-drop response calls `add`, which executes `Command::Append`; the core replaces the supplied placement with the current sequence end. External file drops only import recordings, without the C# timeline insertion path. A drop into an earlier gap therefore cannot preserve the C# behavior. References: [C# drop coordinates](https://github.com/Gimbuhh/Simple-Video-Editor/blob/dd65df13a41490e8168a624b65fcff6a27956940/Development/src/SimpleVideoEditor/Controls/EditorTimeline.cs#L380), [C# insertion](https://github.com/Gimbuhh/Simple-Video-Editor/blob/dd65df13a41490e8168a624b65fcff6a27956940/Development/src/SimpleVideoEditor/MainWindow.xaml.cs#L857), [Rust drop](https://github.com/Gimbuhh/Simple-Video-Editor/blob/c8deaff03c097a7c1af12e199eefa57a50eec9e1/Development/rust/crates/desktop-ui/src/ui.rs#L1379), [Rust append](https://github.com/Gimbuhh/Simple-Video-Editor/blob/c8deaff03c097a7c1af12e199eefa57a50eec9e1/Development/rust/crates/editor-core/src/lib.rs#L490).

These findings follow directly from the production call paths. They are not new end-to-end OS drag/file-association tests. The previous Rust matrix verifies moving existing sections, not this missing recording-drop contract.

**Observed Rust gains:**

- Native display-aware HDR presentation, deliberate SDR tone mapping and HDR10 AV1/HEVC export. The old export service explicitly rejects HDR input. Current-display HDR appearance is user-confirmed; generated HDR exports have media-test evidence.
- Per-section audio selection/gain/mute/fades and saved fit/fill/crop/rotation settings, plus HEVC output and decoded-timestamp VFR stepping. These are implemented media/UI capabilities, not benefits conferred by Rust itself.
- Portable operation without bundling .NET, stronger explicit destination identity/native-tool checks and isolated core/persistence/media owners. FFmpeg and libmpv still perform encoding and decoding.

**Tradeoffs and unverified claims:**

- Rust saves schema 2; C# accepts schema 1 only. Retain original projects for rollback. Successful old-project import does not make newly saved projects backward compatible.
- The 330 Rust desktop checks, 40 scaled layouts, core/media tests and two frozen C# import comparisons establish useful bounded contracts. Their counts cannot establish superiority over the different C# harness's 103 core/media and 233 callback-driven UI checks.
- Windows CI passed the compared Rust source at `c8deaff` (run 39869535291). That does not cover the missing startup/drop contracts identified here.
- No matched export-speed, active-playback latency, cold-start or cold-cache superiority has been established. Identical encoder binaries do not make different filter graphs equivalent workloads.
- The historical Glow debug startup access violation remains unattributed. Physical monitor/HDR-toggle and assistive-technology acceptance remain incomplete. The user-confirmed external Windows/NVIDIA game-switch recording problem is not counted as a Rust defect.

## Resource measurement scope

The comparison runner performs three separate launches per case, alternating case order. Each process settles for ten seconds, then samples CPU time, private committed bytes and working set over six seconds. The cases are empty C#, empty Rust, C# opening a generated 4K SDR project, and Rust holding that recording in native/software preview. Both immutable executables and the generated source retain their hashes; all fifteen helpers close normally. No desktop-control or NVIDIA capture operation is used.

The first hidden-launch measurements are **excluded from the performance verdict**: a follow-up explicitly found C# minimized with a zero-sized client area while Rust had a normal 1360×900 client. A separate C# startup check also produced a non-null **2×2 initial bitmap**; that readiness signal does not establish the final preview resolution during measurement. Repeated sampling alone did not make those workloads comparable. Diagnostic outputs remain private, with their exclusion recorded.

The preview paths are also different benchmark interfaces: C# uses its normal project-opening path, while Rust uses the opt-in hold verifier because normal launch filenames are unsupported. That verifier adds polling/reporting overhead, and default preview sizes/filter graphs differ. Neither process-private memory nor working set includes all dedicated GPU memory. Filesystem cache and other system load are uncontrolled. A visible comparison must verify non-minimized windows and retain these limitations.

### Approved visible comparison

After the user approved visible helper windows, all fifteen visible runs completed with normal shutdown and unchanged source/executable hashes. Every sampled window was visible, non-minimized and at 96 DPI. C# used its default 1304×841 client area and Rust its default 1360×900 area. The following are means across three launches per case on a Ryzen 7 9800X3D with 16 logical processors and an RTX 5070 Ti:

| Visible default-window case | CPU, % of one logical core | Working set, MiB | Private committed memory, MiB |
| --- | ---: | ---: | ---: |
| C# empty project | 0.17 | 126.5 | 128.7 |
| Rust empty project | 33.74 | 154.4 | 571.1 |
| C# generated 4K SDR project | 0.09 | 679.6 | 678.3 |
| Rust native preview hold | 62.27 | 785.2 | 1,328.6 |
| Rust software preview hold | 91.06 | 755.5 | 1,175.3 |

The **empty-project comparison** uses normal app startup without the Rust hold verifier and supports a bounded conclusion: the current Rust app consumes more idle CPU and resident/committed process memory in this setup. Its roughly 34% of one logical core is about 2.1% of the machine's total 16-logical-processor CPU capacity, not 34% of the whole machine. Working set and private committed memory are different measures; these figures do not mean Rust uses four times the physical RAM. Private memory includes driver/backend allocations, and neither column is a complete CPU-plus-GPU footprint.

The preview rows remain default-application-path diagnostics: preview interfaces, dimensions and filter graphs differ, so they do not prove equivalent-frame rendering or export speed. The visible rerun corrects the minimized-window confound but does not establish all other workloads as identical. UI repaint scheduling and playback polling are candidates for profiling; no causal performance fix has been tested.

Visible evidence: version-comparison/aeece648de03426088901718272574c3. The frozen Rust candidate is more capable and smaller, but is **not yet demonstrated to be the better overall replacement**. Higher empty-window idle cost and the two functional gaps should be addressed before retiring C#.

Reproduce diagnostics using `Development/scripts/compare-implementations.ps1 -Source <generated-SDR-recording>`. Add `-Visible` only when visible test windows are wanted; that mode refuses hidden/minimized samples. The runner records executable/native hashes, environment and per-launch window/CPU/memory samples in ignored version-comparison output. The generated source must contain at least 2.5 seconds of video. No private recording is required.

Local evidence: version-comparison/a38f18040b5441e7b59c241d3afac4c7 (15 diagnostic launches, explicit performance exclusion) and version-comparison/7fed6a94735d46c7bc924f6d9bfab29e (five window-state checks). The C# executable was also hashed directly from the checksum-pinned original ZIP and matches the compared app. The original release ZIP SHA-256 remains `5503f27981238c9d38e9e0b31fce8075b0c26511727fdfd16f6c0ef772e86493`. Source/app hashes and repository consistency checks passed; no product code changed.

## Retirement decision

Keep C# main/history, original release, baseline app bundles, original schema-1 projects and the frozen comparison oracle. Before recommending replacement, restore launch-file and timeline-drop parity, reduce the observed empty-window idle costs, and rerun comparison against a new immutable candidate. Then complete the remaining functional acceptance review. Visual refinement remains the user's planned later pass and is not being counted as a current visual win or a request to redesign the interface now. This comparison authorizes no legacy deletion, merge, release tag or publication.
