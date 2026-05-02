# navidrome-tui

Terminal music client for Navidrome/Subsonic API — single Rust binary crate.

**Language: Rust only.** All code, tests, scripts, and configuration must be written in Rust. Do not suggest or generate code in any other language.

## Setup & Build

```bash
cargo run              # debug build (default: tls-native, needs OpenSSL)
cargo build --release
cargo fmt              # rustfmt.toml: edition=2021, use_small_heuristics=Max
cargo clippy           # lint (aim for zero warnings on new code)
```

- **default** features = `tls-native` (requires system OpenSSL + pkg-config)
- `cargo build --no-default-features --features tls-rustls` — no system TLS deps (used in CI)
- No test suite exists.

## Architecture

| Module | Purpose |
|--------|---------|
| `src/main.rs` | Entry point, panic hook, event loop |
| `src/client.rs` | HTTP client for Navidrome/Subsonic API |
| `src/database/` | SQLx + SQLite; migrations at `src/database/migrations/` (auto-run on startup) |
| `src/mpv.rs` | Playback via libmpv2 |
| `src/player.rs` | Queue, repeat, shuffle logic (methods on `App`) |
| `src/tui.rs` | Ratatui + Crossterm UI (~2.5k LOC, main `App` struct) |
| `src/themes/` | Theme engine with album-art color extraction |
| `src/keyboard.rs` | Key binding map, input routing |
| `src/popup.rs` | Popup menus (context, global, search) |
| `src/helpers.rs` | `State`, `Preferences`, scrollbar rendering |
| `src/library.rs` | Library browsing + cover art rendering |
| `src/extra/` | Linux `.desktop` file |

## Windows-specific

- `build.rs` adds `cargo:rustc-link-search=native=libmpv`. The `libmpv/` dir must contain `mpv.lib` and `mpv.def` (both committed).
- `libmpv-2.dll` is required at runtime but **not committed** (`.gitignore` excludes `*.dll`).
- Platform deps: `windows-sys`, `windows`, `souvlaki` (media keys).

## Config

YAML at platform data dir (`%APPDATA%\navidrome-tui\config.yaml` on Windows, `~/.config/navidrome-tui/config.yaml` on Linux). Supports `$VAR` / `${VAR}` / `%VAR%` env substitution. See `README.md` for full schema.

## Nix

`shell.nix` and `flake.nix` provide dev shells on Nix-enabled systems. Not needed on Windows.

## CI

Release workflow (`.github/workflows/release.yml`) builds Linux x86_64 and macOS aarch64 with `tls-rustls`. Packages binary as `jellyfin-tui` (fork artifact — the real binary name is `navidrome-tui`).

## Code Style & Best Practices

### Error handling

- Return `Result<T, E>` for fallible operations. Avoid `unwrap()` / `expect()` outside of startup/bootstrap code.
- **Never** use `unwrap()` on I/O or serialization in `database/extension.rs` — that module is particularly prone to data-corruption crashes. Use `?` propagation.
- `panic!` is only acceptable for truly unrecoverable startup errors (e.g. missing data directory).

### Borrowing & cloning

- Prefer `&str` over `String`, `&[T]` over `Vec<T>` in function parameters.
- Avoid `.clone()` on IDs when a reference suffices. The codebase currently has ~300 clone calls, many redundant. When adding new code, pass `&str` or use `as_str()` instead of cloning IDs.
- For a single element, prefer `std::slice::from_ref(track)` over `&[track.clone()]`.

### Iterators & patterns

- Use `.map()` instead of `if let Some(x) = ... { Some(x) } else { None }`.
- Use `.is_some()`, `.is_none()`, `.is_ok()`, `.is_err()` instead of `if let Some(_) = x` / `if let Ok(_) = x` / `if let Err(_) = x`.
- Use `.is_some_and(|x| condition)` instead of `.map_or(false, |x| condition)`.
- Use `.is_none_or(|x| condition)` instead of `.map_or(true, |x| condition)`.
- Prefer `unwrap_or_default()` over `unwrap_or_else(|| Default::default())`.
- Use `sort_by_key()` instead of `sort_by()` when the key type implements `Ord`.
- Use `.iter()` over `.into_iter()` on references to avoid confusion.

### Match ergonomics

- Collapse `match` arms with nested `if` into match guards: `ActiveTab::Library if !self.tracks.is_empty() => { ... }`.
- Replace single-arm `match` with `if` when checking one variant.

### Linting

- No `return;` at the end of a block — `needless_return`.
- No unnecessary borrows in function calls — pass owned values directly when the function takes them by value.
- Use `#[derive(Default)]` for enums/structs with straightforward defaults instead of manual `impl Default`.
- Run `cargo fmt` before committing.

### Documentation

- `//` comments explain *why* (design rationale, workarounds).
- `///` doc comments explain *what* public APIs do.
- Every `TODO` should reference a GitHub issue: `// TODO(#issue): description`.

### Testing

- No test suite exists yet. When adding tests, name them descriptively: `queue_should_skip_played_tracks_when_shuffle_enabled()`.
- Test commands ran in CI: `cargo build --release` (no test step).

### Apollo GraphQL Rust Best Practices (Local Skill)

- **Mandatory Context**: This repository includes a local copy of Apollo GraphQL's Rust Best Practices in the `rust-best-practices/` directory.
- **Agent Instruction**: Whenever writing, refactoring, or reviewing Rust code, you MUST first consult `rust-best-practices/SKILL.md` and read the relevant chapters in `rust-best-practices/references/` to ensure the code adheres to these guidelines.
