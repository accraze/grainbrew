# Changelog

All notable changes to Grainbrew are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-11-09

### Added
- Time-domain granular engine with no FFT or lookahead, reporting 0 samples of latency.
- Timbre (±24 st), Spread (per-grain pitch, pan and polarity), Scatter (0–250 ms), Density
  (5–500 /s) and Size (2–50 ms).
- Freeze: holds the input so the cloud keeps playing it, with the dry signal still following Mix.
- Tone tilt EQ, Drive soft clipping, Mix and Output.
- Bypass with a click-free crossfade and host bypass integration.
- Knob GUI.

[Unreleased]: https://github.com/accraze/grainbrew/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/accraze/grainbrew/releases/tag/v0.1.0
