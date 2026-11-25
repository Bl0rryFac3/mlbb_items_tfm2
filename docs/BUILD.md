# Build requirements

The native six-slot/build hooks intentionally retain upstream's actual engine types, signature validation, memory patches and exact game 0.7.4 version gate. Do not replace the hook with a hardcoded success result or guessed ABI mirror.

Required:

- Windows x86-64 MSVC Rust toolchain **nightly-2026-05-24**: `rustc 1.98.0-nightly (23a3312d9 2026-05-23)`.
- Stable SDK crate `mod-api-stable` beside the source folder; copy the `mod-api-stable` crate from the game’s installed `mod-sdk-stable` directory so that `../mod-api-stable/Cargo.toml` exists.
- Official legacy game SDK **0.5.2**, specifically `mod-sdk/deps/libgame_core-ca200d77d5c563ee.rlib`, all sibling dependencies and the `mod-sdk/native` import libraries. Download from https://github.com/teamsamoyed/TeamfightManager2Mod/releases/download/0.5.2/0.5.2.zip .
- MSVC linker/CRT/Windows SDK, or rust-lld with equivalent libraries.

PowerShell example, with paths adjusted:

```powershell
$legacySdk = 'C:\TFM2SDK\mod-sdk'
$env:CARGO_ENCODED_RUSTFLAGS = @(
    '-L', "dependency=$legacySdk\deps",
    '--extern', "game_core=$legacySdk\deps\libgame_core-ca200d77d5c563ee.rlib",
    '-L', "native=$legacySdk\native"
) -join [char]31
cargo +nightly-2026-05-24 test --lib --release --locked
cargo +nightly-2026-05-24 build --release --locked
```

Output: `target/release/mlbb_items.dll`. The required exports are `tfm2_mod_entry_stable` and `tfm2_mod_required_abi_level`. Package it with this fork's `mod.mod_info`, `mod.override_info`, runtime asset directories and text files. Do not rename an upstream or Arena DLL and ship it with this catalog.

The SDK dependency is deliberately not bundled as 1.5 GB of compiled artifacts; its version, exact metadata filename and official download are recorded for reproducibility. Linux cross-checking alone cannot load the SDK's Windows procedural macro dependencies. The build in this session ran the pinned Windows compiler under Wine with the actual SDK and Windows import libraries.
