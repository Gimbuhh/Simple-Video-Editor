# Rust migration workspace

This is the first foundation for the planned 1.1.0 rewrite, not a replacement
desktop editor. See the [plan](../../docs/rust-rewrite-plan.md) and
[status/contract ledger](../../docs/rust-rewrite-status.md).

Run from the repository root on Windows x64:

```powershell
./Development/scripts/setup-rust.ps1
./Development/scripts/test-rust.ps1

# After setup.ps1 restores the C# baseline tools:
./Development/scripts/test-rust.ps1 -LegacyParity

# Inspect an existing project without writing it or opening its recordings:
./Development/rust/target/release/sve-inspect.exe 'C:/projects/example.sveproject'
```

`editor-core` owns pure timeline rules. `project-store` imports schema-1 data
without touching media. `sve-inspect` prints a path-free JSON timeline summary.
The test-only C# oracle compares committed generated-data fixtures against the
unchanged production reader. It is never a runtime dependency of Rust.

Legacy floating-point trim values remain intact. Rational frame rates describe
new split constraints; this is not a finished rational/VFR timing engine.
Recording syntax checks are not filesystem authorization: a future Windows
adapter must validate drives and each link target before any media access.
No save/upgrade/export/recovery path is present.

Rust 1.90.0, bootstrap checksum and Cargo.lock are pinned. Tools, downloaded
crates and build output stay ignored inside the workspace. GNU is sufficient
for this pure foundation; the rendering spike will settle production tooling.
The workspace version 0.1.0 identifies private tools; the C# project still owns
shipping version 1.0.4 until verified cutover to application version 1.1.0.

Workspace source uses the repository MIT license. Locked crates retain their own
license terms; inspect `cargo metadata --locked --offline` for exact versions and
license expressions. Runtime dependencies include Serde/serde_json (MIT or
Apache-2.0), memchr (Unlicense or MIT), itoa (MIT or Apache-2.0), and ryu
(Apache-2.0 or BSL-1.0). Build-time unicode-ident additionally requires Unicode-3.0.
The future Rust portable package must include the matching notices and runtime
toolchain licenses before distribution; the current App bundle contains no Rust.
