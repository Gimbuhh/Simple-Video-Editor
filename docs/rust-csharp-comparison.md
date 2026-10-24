# C# 1.0.4 versus Rust 1.1.0

Compared on 2026-10-23. The improved Rust candidate restores the identified startup/drop contracts, removes continuous idle work and adds HDR, audio and framing capabilities beyond C#. Its download is about 25% smaller. It is a stronger functional upgrade, with remaining resource and acceptance tradeoffs stated below. Visual refinement is the user's separate later pass.

## Exact application targets

| Artifact | Preserved C# | Improved Rust |
| --- | --- | --- |
| Application source | `dd65df13a41490e8168a624b65fcff6a27956940` | `30357523b854b0d8ce82ce689c748c54ed414a32` |
| Embedded executable version | 1.0.4.0 | 1.1.0.0 |
| Executable SHA-256 | `703eb67be5b6ff6035ac06de2f0a7dec06685299643e7d5ab26dc4e46e129cf6` | `d17a3bf01c215171605f9505eba825b82a0f3daf50b020d8b3e875902e2598e8` |
| Executable bytes | 140,125,284 | 23,296,512 |

The complete portable bundles are approximately 469 MB and 367 MB; their downloads are approximately 199 MB and 149 MB (decimal MB). This is about 22% less extracted storage and 25% less download, rather than the much larger reduction suggested by executable sizes alone. Final ZIP checksums live in their sidecars. All compared media binaries match by SHA-256. Both distributions are portable; C# already bundled its runtime, so neither requires a separate .NET installation.

C# main/history and its original release ZIP are retained. Redundant extracted baseline bundles and cached source were removed during the requested cleanup; extract the ZIP or restore the pinned Git commit when needed. The original ZIP SHA-256 is `5503f27981238c9d38e9e0b31fce8075b0c26511727fdfd16f6c0ef772e86493`. The table records the measured executable before the icon and UI refinements. The current review build includes the selected scissors/play icon and a first pass removing redundant UI copy; the resource measurements have not been repeated for those changes.

## Restored behavior and performance changes

- **Filename startup works.** A project opens through the normal application path with its saved library, trims, gaps and settings. Multiple recording filenames import in order. Relative filenames and Unicode/space paths are covered. Explicit files take priority over recovery prompts without discarding another interrupted session. This restores executable-drop/command-line behavior; it does not claim that either app registers Windows file associations.
- **Timeline drops retain their position.** Library and File Explorer drops place linked sections at the pointer, using the nearest free position and each source's frame grid, even when the output rate differs. Existing sections and original trim values stay intact; a linked insertion batch has one undo step. Dropping elsewhere imports into the library. Core tests cover mixed rates, overlap resolution, gaps and undo; actual egui library/external-file drops cover the production dispatch.
- **Idle work sleeps.** Background events wake the UI, paused playback checks display/HDR state once per second, and new requests/shutdown wake the actor immediately. Software rendering waits for callbacks or explicit refresh/resize. The player is created only for a recording, and new projects release it. Cached native bounds/title updates avoid repeatedly invalidating Windows. DX12 uses a memory-oriented allocation hint.
- **Preview processing follows the displayed area.** Preview transforms use size tiers, while source timestamps, project settings and export resolution stay unchanged. The same color/framing implementation remains authoritative. Native failures retain the dequeued/latest queued request for software fallback; a controlled failure-after-dequeue test exercises that recovery.

Production owners: [startup parsing](https://github.com/Gimbuhh/Simple-Video-Editor/blob/30357523b854b0d8ce82ce689c748c54ed414a32/Development/rust/crates/desktop-ui/src/startup.rs), [UI dispatch](https://github.com/Gimbuhh/Simple-Video-Editor/blob/30357523b854b0d8ce82ce689c748c54ed414a32/Development/rust/crates/desktop-ui/src/ui.rs), [timeline commands](https://github.com/Gimbuhh/Simple-Video-Editor/blob/30357523b854b0d8ce82ce689c748c54ed414a32/Development/rust/crates/editor-core/src/lib.rs), [playback actor](https://github.com/Gimbuhh/Simple-Video-Editor/blob/30357523b854b0d8ce82ce689c748c54ed414a32/Development/rust/crates/desktop-ui/src/playback.rs). The reference drop policy is [C# TimelineLayout](https://github.com/Gimbuhh/Simple-Video-Editor/blob/dd65df13a41490e8168a624b65fcff6a27956940/Development/src/SimpleVideoEditor/TimelineLayout.cs).

## Visible resource comparison

| Visible paused/default-window case | CPU, % of one logical core | Working set, MiB | Private committed memory, MiB |
| --- | ---: | ---: | ---: |
| C# empty project | 0.34 | 119.2 | 127.6 |
| Rust empty project | 0.00 | 143.9 | 251.5 |
| C# generated 4K SDR project | 0.34 | 672.4 | 677.2 |
| Rust native preview of that project | 0.26 | 760.0 | 893.5 |
| Rust software preview of that project | 0.09 | 724.4 | 838.3 |

Means are from three launches per case on a Ryzen 7 9800X3D (16 logical processors) and RTX 5070 Ti. Each settles for ten seconds and samples process CPU, working set and private committed memory for six seconds. All fifteen helpers must be visible, non-minimized and close normally; executable/native/source hashes are retained. The same generated 4K SDR recording and project (0.5-2.5 seconds retained at timeline zero) open through each app's ordinary filename path. Separate launch/desktop and 4K readiness checks establish that these paths work; the sampler does not continuously instrument rendered frames.

CPU percentages describe one logical core; divide by sixteen for this machine's overall CPU capacity. A zero reading means no CPU-time increment was recorded in the short sampling interval, not literally zero instructions. One 15.625 ms CPU tick in six seconds is about 0.26% of one core. Working set and private commit are different measures, and neither includes all dedicated GPU memory. C# uses a 1304x841 client and Rust 1360x900, all at 96 DPI. Default preview dimensions/backends, OS cache and other system load differ. These are bounded default-app idle measurements, not matched export speed, active playback throughput or cold-start tests.

The original Rust review artifact (`b6404bc435ff381f166dbfd8ed36cc72f2fce58b699881fef2333a9e8d28034a`, source `c8deaff`) used **33.74% of one core, 154.4 MiB working set and 571.1 MiB private commit** at normal empty-project startup. The improved candidate removes that observed continuous idle cost. Earlier preview-hold measurements have a different verification interface and are excluded from direct before/after preview claims. The first hidden comparison is also excluded: C# was minimized while Rust was not.

The final Rust empty-window private commit is about 56% below the earlier Rust candidate, while its working set is about 7% lower. This is not a claim that physical RAM halves. All final cases have low idle CPU; differences between the C# and Rust readings are near the sampling floor and do not establish a general speed advantage. Memory remains higher than C# here: about 25 MiB more working set when empty, 52 MiB more in software preview and 88 MiB more in native preview. The rewrite therefore does not establish a memory-use win over C#.

Final visible evidence: version-comparison/01c3189ec1144935bb10c162aac37f73. Original visible evidence: version-comparison/aeece648de03426088901718272574c3. Excluded hidden diagnostics: version-comparison/a38f18040b5441e7b59c241d3afac4c7 and version-comparison/7fed6a94735d46c7bc924f6d9bfab29e. Intermediate optimization samples are retained privately but are not the final-artifact comparison.

Reproduce with `Development/scripts/compare-implementations.ps1 -LegacyApp <extracted-CSharp-1.0.4-bundle> -RustApp App -Source <generated-SDR-recording> -Visible -OrdinaryStartup`. Visible mode refuses hidden/minimized samples. It closes only its own children and records raw samples, means, hashes and window states. No private recording, desktop-control session or NVIDIA capture API is required.

## Verified advantages and remaining tradeoffs

Rust offers native display-aware HDR preview, SDR conversion and HDR10 AV1/HEVC export; C# explicitly rejects HDR input. It also provides per-section audio selection/gain/mute/fades, saved fit/fill/crop/rotation, HEVC output and decoded-timestamp VFR frame navigation. These are implemented capabilities with media/UI evidence, not benefits inferred from the implementation language. Current-display HDR appearance is user-confirmed; metadata and screenshots alone do not certify physical HDR.

The corrected executable passes strict formatting/Clippy, 36 workspace/executable tests, 48 native lifecycle/backend checks, five GPU/media integrations and three normal filename-startup cases. Its relocated portable bundle passes 344 desktop checks in sixteen SDR/PQ/HLG, geometry and VFR runs; controlled native failures pass fifty checks; forty scaled layouts pass forty-seven checks; twenty repeated PQ startup/import/seek/edit/shutdown cycles pass. These counts establish the named contracts, not superiority over a different C# test suite. The [status ledger](rust-rewrite-status.md) records exact artifacts and scope.

Separate checks of this exact executable confirm ready 4K SDR preview on the requested native and software backends, and ready native PQ preview with Windows HDR active and `pq` output parameters. All three helpers close normally and preserve source/executable hashes. Evidence: optimized-preview-hold/8336dc65e4b3458d96778474a0b3e838. These checks establish backend/output state, not physical luminance or color appearance.

Newly saved projects use schema 2; C# reads schema 1 only. Keep original projects for rollback. Broader physical monitor/HDR toggles, real DPI transitions and assistive-technology acceptance remain incomplete. An old Glow debug startup failure is still unattributed; the production UI uses DX12 and current stress runs pass. The user confirmed that the separate Windows/NVIDIA game-switch recording issue is external; that investigation remains closed.

The candidate is more capable and smaller, restores the known interaction gaps and avoids the previous idle polling cost. It is a practical upgrade for the tested HDR/editing workflows, with the memory tradeoff above. C# history, the original release ZIP, comparison reports and original projects remain available through user acceptance. Visual refinement and transition effects remain later work. PR #8 stays open; no merge, release tag or publication is performed.

## Timeline and preview follow-up — 2026-10-24

Timeline filmstrips and floating library-drag thumbnails are implemented. Same-presentation seeks retain their image and HDR indicator. Continuous pointer movement now allows frames to finish, coalesces pending targets and seeks the exact final frame on release. Native AV1 can use copied hardware frames; other codecs and Software preview retain CPU decoding after a generated HEVC case regressed with hardware decoding. Preview-only downscaling precedes color processing; full-resolution export stays unchanged.

A separate comparison uses the prior Rust engine at `9edaae1` plus verification-only timing instrumentation (executable SHA-256 `96d75585b5743390a16c1c84617cb7dc0bf042c4eef4542b68cbb56a3748beb6`) and the final executable (`8dfd419ab8266e1bdc777803688f03b77e927e5c2cd525056979654241c65dc5`). These are Rust before/after measurements, not new C# resource or speed results. Isolated runs complete 24 exact seeks each; source/executable hashes are checked. Recordings remain unchanged and are excluded from source and release packages.

| Recording and preview | Prior Rust median / p95 | Updated Rust median / p95 |
| --- | ---: | ---: |
| Supplied 2560×1440, approximately 60 fps AV1/HDR, native | 280.9 / 469.4 ms | 100.0 / 150.0 ms |
| Same recording, Software preview | 278.6 / 468.4 ms | 215.3 / 369.4 ms |

Native AV1 median falls about 64%, with d3d11va-copy reported; Software preview stays CPU-only. Generated 4K HEVC with a ten-second GOP still takes 575.0/916.6 ms median/p95. HEVC timings vary with system load, so no substantial HEVC or universal speedup is established. The new continuous-drag test presents 18/8/4 completed frames during separate two-second native AV1/software AV1/native HEVC gestures and verifies exact release settling. A negative control using the prior cancellation policy completed zero HEVC frames, demonstrating the starvation regression.

The helper times UI request to production playback-ready acknowledgement; Software preview requires a post-seek rendered frame, while native acknowledgement is not a physical presentation measurement. It checks retention during pending seeks. Filesystem caching and system load remain uncontrolled. Use `Development/scripts/benchmark-preview.ps1 -AppPath <bundle-executable> -Source <local-recording>`; add `-Software` for that fallback and run after other desktop/media work finishes. Raw reports and hashes are retained privately under preview-benchmark, indexed by timeline-verification.json. The [rewrite ledger](rust-rewrite-status.md) records final validation. Earlier C# resource measurements and remaining physical acceptance limits above remain applicable.

## Ordinary SDR seeking follow-up — 2026-10-24

The pinned 1.0.4 implementation sends exact seeks through libmpv and coalesces pointer requests on a 33 ms timer. Its SDR software renderer uses the sw-fast profile and has no project video-filter graph. The rewrite initially applied its export-oriented color/framing graph to every preview, including ordinary unchanged SDR. That extra processing also prevented direct GPU surfaces from reaching the renderer. The difference is in the preview pipeline; both versions use libmpv.

The updated 1.1 preview bypasses CPU pixel processing for ordinary Rec.709 YUV 4:2:0 SDR footage with square pixels, no source/section rotation, Fit framing and matching source/output aspect. Explicit metadata retains the shared graph's defaults for missing color tags. The renderer scales the image; native preview can decode directly to D3D11 surfaces, with automatic CPU fallback. HDR, other color spaces, cropping/fill, rotated/anamorphic footage and aspect changes retain the shared transform. Export is unchanged. The metadata-only behavior of mpv's format filter is documented in the [mpv manual](https://mpv.io/manual/stable/#video-filters).

Sequential runs on the same generated 3840×2160 HEVC recording with a ten-second GOP use the same 24 exact target positions. The C# reference executes the original playback and renderer classes from `dd65df1`, with only DLL resolution and a restart counter instrumented, at 960×540. It waits for seek restart, a new software frame and a timestamp within one source frame; it excludes WPF input/presentation and startup. Rust measures the production app request/acknowledgement path, with a post-seek rendered frame for software. These are bounded pipeline comparisons, not matched full-interface or physical-display latency measurements.

| Path | Median / p95 exact seek |
| --- | ---: |
| Pinned 1.0.4 C# playback classes, software | 514.9 / 855.9 ms |
| Updated 1.1, Software preview | 502.7 / 827.3 ms |
| Updated 1.1, native direct GPU preview | 332.0 / 541.5 ms |

Software preview is comparable to the C# reference in this case; the small difference does not establish a general CPU speed win. Native uses d3d11va, with a median about 42% below the previous 1.1 build's 575.0 ms on the same fixture. Native and software present 7 and 5 completed frames during their two-second pointer gestures and settle exactly on release. The supplied AV1/HDR recording retains the processed path and passes at 93.2/136.2 ms native and 216.6/353.0 ms software. Filesystem caching and system load are uncontrolled; long-GOP exact decoding still takes time. This does not establish parity for every C# workflow or physical HDR/display combination.

Final executable SHA-256: `db9cea859d2d851457650cf3cdec931aeed09f66eb64489575699e03915a9f5e`. Final package verification includes 159 desktop checks and 56 injected-fallback checks; the extended matrix passes 386 checks on the behaviorally identical build before a comment-only edit. Six media integrations and 38 workspace/executable tests pass. SDR integration checks compare 8/10-bit full/limited-range primary colors, black/white and gray steps against the existing transform, including missing tags, software and direct GPU paths, exact seek position and source preservation. Reproduce with benchmark-legacy-preview.ps1 and benchmark-preview.ps1 after other verification finishes. Evidence is indexed privately in seek-parity/verification.json; original source/native hashes and raw reference samples are in legacy-seek/83897dcd10174299a3403b105e8a8ba6.
