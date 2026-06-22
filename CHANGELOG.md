# Changelog

All notable changes to this project are documented in this file. The format follows Keep a Changelog, and the project uses semantic versioning.

## [Unreleased]

### Added
- Live dashboard with CPU, RAM, and disk gauges that recolor by load.
- Per core load row and rolling history graphs for CPU and network.
- Windows hardware detail through WMI: GPU, display resolution, and battery.
- Live network throughput for download and upload.
- Theme system with five presets: windows, matrix, dracula, nord, amber.
- TOML configuration with color overrides and section toggles.
- Static snapshot mode via `--once`, plus `--theme`, `--config`, and `--gen-config`.
- Glyph safe rendering with optional fancy icons in capable terminals.

### Notes
- First public preview. Interfaces may change before 1.0.
