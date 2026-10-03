# Contributing to MoMo

For setup and build commands, see [README.md](README.md). For integration payloads, see [docs/AGENTS.md](docs/AGENTS.md).

## Development

- Keep changes focused and include reproduction steps for bug fixes.
- For macOS, edit `NotchBuddy/project.yml` and run XcodeGen; do not edit the generated Xcode project manually.
- Keep API credentials in the platform credential store. Never commit secrets or local configuration.
- Make network requests only to services the user configures.
- Keep hooks responsive when the app is unavailable.
- Changes to Claude Code settings must preserve existing hooks, show a diff and create a backup before the user confirms the change.
- Respect the separate artwork and sound license in `LICENSE-ASSETS.md`.

## Validation

For Windows/Linux changes, run `npm.cmd run build`, `npm.cmd run test:ui`, `cargo fmt --all -- --check` and `cargo test --workspace --release --locked` from `windows/`. Install the UI test browser once with `npx.cmd playwright install chromium`. AI tests use simulated IPC/HTTP/vault data and must never depend on personal credentials.

For macOS changes, build the generated project and run the screen geometry and safe-link checks described in the README.

Include a screenshot or short recording for visual changes. Describe what changed and how it was verified.
