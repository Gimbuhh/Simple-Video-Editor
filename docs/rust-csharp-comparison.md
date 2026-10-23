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
