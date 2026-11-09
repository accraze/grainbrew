# Grainbrew

A zero-latency, time-domain granular timbre shifter (VST3 / CLAP), built with
[nih-plug](https://github.com/robbert-vdh/nih-plug).

Short Hann-windowed grains are read from the recent input and resampled without compensating their
duration, so the spectrum shifts instead of the grains being re-timed. There is no FFT or lookahead,
and the plugin reports 0 samples of latency.

| Control | Range         | What it does                                                        |
|---------|---------------|---------------------------------------------------------------------|
| Timbre  | -24 to +24 st | Resampling rate of each grain                                       |
| Spread  | 0–100%        | Per-grain random pitch (up to ±12 st), pan and polarity             |
| Density | 5–500 /s      | Grains started per second, with jittered spacing                    |
| Size    | 2–50 ms       | Grain length before resampling                                      |
| Tone    | -1 to 1       | Tilt EQ on the grains around 1 kHz, ±6 dB                           |
| Drive   | 0–100%        | Soft-clip saturation on the grains                                  |
| Mix     | 0–100%        | Dry/wet                                                             |
| Output  | -24 to +12 dB | Final level                                                         |

Knobs: drag or scroll to turn, hold Shift for fine control, double-click to reset.

Grains can only read input that has already arrived, so a grain sped up by Timbre starts further
back in the history. At +24 st with 50 ms grains the wet signal trails by up to about 47 ms. The dry
path is never delayed.

## Build

Needs a recent stable Rust from [rustup](https://rustup.rs).

```sh
cargo xtask bundle grainbrew --release
cargo test
```

Bundles land in `target/bundled/`. On Linux, the GUI also needs X11/GL headers:

```sh
sudo apt install libx11-xcb-dev libxcb1-dev libxcb-dri2-0-dev libxcb-icccm4-dev libxcursor-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl-dev
```

## Install

### macOS

Install the Xcode command-line tools (`xcode-select --install`), build, then:

```sh
cp -r target/bundled/Grainbrew.vst3 ~/Library/Audio/Plug-Ins/VST3/
cp -r target/bundled/Grainbrew.clap ~/Library/Audio/Plug-Ins/CLAP/
```

Apple Silicon needs code to be signed, and an unsigned local build may be refused by the host. Sign
it ad hoc after copying:

```sh
codesign --force --deep -s - ~/Library/Audio/Plug-Ins/VST3/Grainbrew.vst3
codesign --force --deep -s - ~/Library/Audio/Plug-Ins/CLAP/Grainbrew.clap
```

For a universal Intel + Apple Silicon build, add both Rust targets and run
`cargo xtask bundle-universal grainbrew --release`.

### Windows

Install the MSVC build tools (the "Desktop development with C++" workload), build, then in an
elevated PowerShell:

```powershell
Copy-Item -Recurse target\bundled\Grainbrew.vst3 "$env:COMMONPROGRAMFILES\VST3\"
New-Item -ItemType Directory -Force "$env:COMMONPROGRAMFILES\CLAP"
Copy-Item -Recurse target\bundled\Grainbrew.clap "$env:COMMONPROGRAMFILES\CLAP\"
```

Rescan plugins in your host afterwards.

### Linux

```sh
cp -r target/bundled/Grainbrew.vst3 ~/.vst3/
cp -r target/bundled/Grainbrew.clap ~/.clap/
```

## License and the VST3 caveat

Grainbrew is licensed under GPL-3.0-or-later (see `LICENSE`).

nih-plug itself is ISC-licensed, but the VST3 bindings behind `nih_export_vst3!` are GPLv3. Any VST3
build of a nih-plug plugin must therefore be distributed under GPLv3 terms, including providing the
source. A closed-source release would have to drop the VST3 target (remove `nih_export_vst3!` and
the `Vst3Plugin` impl, and ship CLAP only) or replace those bindings. Grainbrew's own GPL license
would also need to change for that.
