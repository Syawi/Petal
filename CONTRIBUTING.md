# Contributing

Use Windows 11 x64, Rust 1.89 or newer, and either the MSVC build tools or a working MinGW-w64 GNU toolchain.

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

Successful conversion saves a verified copy alongside the original. Always preserve the original byte-for-byte, keep collision handling, and do not silently discard unsupported content.

Keep the interface, messages and documentation in English. Preserve native rendering, whole-pixel text, reduced-motion behavior and the six-petal maximum. Include a focused test for changes to parsing, conversion or source preservation. Describe the visible behavior and validation in pull requests.

Do not commit media executables, target/, local logs, personal files, tokens or build artifacts. The pre-generated .res/.o icon resources in assets/ are intentional source assets.

See [TESTING.md](TESTING.md) for native and manual checks and [docs/MEDIA.md](docs/MEDIA.md) for reproducible README visuals.
