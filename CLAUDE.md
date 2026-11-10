# Grainbrew

Granular cloud effect (CLAP/VST3) in Rust using nih-plug. The user runs it in Bitwig Studio on Linux (CLAP).

## Commands

```sh
cargo xtask bundle grainbrew --release
cp target/bundled/Grainbrew.clap ~/.Grainbrew.clap.new && mv ~/.Grainbrew.clap.new ~/.clap/Grainbrew.clap   # install
cargo test                          # debug on purpose, see below
cargo clippy --release --all-targets
```

- The system has an old distro Rust in `/usr/bin` ahead of `~/.cargo/bin`. Prefix commands with `PATH="$HOME/.cargo/bin:$PATH"`.
- Install by rename, as above: overwriting the file in place can crash a running Bitwig that has it loaded.
- Bitwig needs the device deleted and re-added to pick up a new build. If a brand-new plugin doesn't appear, quit Bitwig and move `~/.BitwigStudio/cache/clap-metadata-13` aside.
- Add user-facing changes to `CHANGELOG.md` under `[Unreleased]`.
- Releases: follow `docs/RELEASING.md`. Never move an existing tag.
- The "trailing semicolon in macro" warnings come from nih-plug's export macros.

## Structure

- `src/dsp/` has no nih-plug code, and `Engine::process` is the whole signal path. `tests/render.rs` renders it offline.
- Debug tests matter: `assert_no_alloc` only aborts in debug (nih_plug installs its allocator there), and `InputRing::read` debug-asserts that no grain reads the future or past the oldest sample.
- Grains play with no duration compensation, so a faster grain is a shorter one. Each starts far enough back to never pass the input. Under Freeze the input stops, so grains fit their whole source span into what's held, and fast grains already playing park at the end.
- Overlap gain is `1 / sqrt(max(N * 3/8, 1))`, which assumes out-of-phase grains. With Timbre and Scatter both 0 grains line up and run up to about 7 dB hotter.
- Timbre, Spread, Scatter, Density and Size are unsmoothed because they're only read at grain start.
- The audio thread must not allocate. Everything is allocated in `Engine::new`, called from `initialize()`.
- `CLAP_ID` and `VST3_CLASS_ID` must never change, or saved projects lose the plugin.
