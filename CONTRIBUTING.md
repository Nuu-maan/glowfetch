# Contributing

Thank you for your interest in glowfetch. Contributions are welcome.

## Getting started

1. Install the stable Rust toolchain on the MSVC target.
2. Clone the repository and build with `cargo build`.
3. Run the dashboard with `cargo run`.

## Development guidelines

- Keep the binary glyph safe. Anything beyond full blocks and box drawing must be gated behind the fancy flag, because the classic Windows console lacks many glyphs.
- Prefer data sources that work without elevation.
- Match the existing code style. Run `cargo fmt` before committing.
- Run `cargo clippy` and resolve warnings where reasonable.

## Pull requests

- Describe the change and the motivation.
- Keep each pull request focused on one topic.
- Update the changelog when behavior changes.

## Reporting issues

Open an issue with your Windows version, terminal, and steps to reproduce. Screenshots help a great deal for rendering problems.
