# MoMo

MoMo is a desktop companion that displays coding sessions, permission requests, chats, file drops and service integrations at the top of your screen.

This repository is a rebrand of [Coucou](https://github.com/Louis-CFM/coucou), based on its 0.1.2 source archive. The Windows/Linux package version is 0.4.0. See [PROVENANCE.md](PROVENANCE.md) for the origin and changes.

Windows/Linux chat supports Claude, OpenAI / ChatGPT, Gemini, other OpenAI-compatible APIs and local models through Ollama or a local OpenAI-compatible server. Each model profile has its own ordered API keys and optional model fallbacks. See [the AI setup guide](docs/AI-MODELS.md) and [CHANGELOG.md](CHANGELOG.md).

Settings includes a local model setup panel that reuses downloaded Ollama models and starts a server from the chosen folder. Optional AI desktop actions can open supported apps and save notes in your text editor. The chat shows actual action results. Large model files remain local and are excluded from this repository and its installers.

## Windows installation

Run the MoMo Windows x64 installer on Windows 10 or 11. The single installer includes Microsoft WebView2 for installation without internet when the runtime is missing. Target devices do not need Node.js, Rust or build tools.

Compiled installers and local tools are excluded from Git. The **Windows** workflow can generate an installer artifact from **Actions → Windows → Run workflow**. Each version folder contains only its installer and README. MoMo installers are currently unsigned.

See [PANDUAN-WINDOWS.md](PANDUAN-WINDOWS.md) for installation and [windows/README.md](windows/README.md) for usage.

## Build from source

### Windows

Install Node.js 22, stable Rust with the MSVC toolchain, Microsoft C++ Build Tools and WebView2. Follow the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

```powershell
git clone https://github.com/ZuanCrisp/MoMo.git
cd MoMo/windows
npm.cmd ci
npm.cmd run tauri dev
```

Build an installer:

```powershell
npm.cmd run pack
```

Output is written to `windows/release/<version>/`. Packaging downloads the official WebView2 installer to include it in the setup file; app features that call online services still require internet.

If this checkout has a portable toolchain under `.tools/`, use `.\npm-momo.cmd` in place of `npm.cmd`. The helper also supports an installed toolchain.

### macOS

Install Xcode and XcodeGen, then generate the project:

```bash
cd NotchBuddy
xcodegen
open NotchBuddy.xcodeproj
```

Select your own signing team for local signed builds. Edit `project.yml` and regenerate instead of editing the generated Xcode project.

### Linux

The Tauri source and platform instructions are in [windows/README.md](windows/README.md).

## Validation

From `windows/`:

```powershell
npm.cmd run build
npx.cmd playwright install chromium
npm.cmd run test:ui
cargo fmt --all -- --check
cargo test --workspace --release --locked
```

On macOS, run `bash scripts/test-screen-geometry.sh` and `bash scripts/test-safe-links.sh` from the repository root.

## Layout

- `windows/`: shared Tauri frontend, Rust backend, hook relay and packaging scripts.
- `NotchBuddy/`: native macOS application and shared resources.
- `docs/`, `design/`: integration documentation and interface references.
- `scripts/`, `tests/`: macOS build helpers and checks.
- `.github/workflows/`: build validation and manual Windows installer artifacts.

Local credentials, toolchains, archives, installed apps and build outputs are ignored.

## License and credits

The source code retains Louis Raillé's [MIT license](LICENSE). Original character artwork, icons, sounds and media have separate terms in [LICENSE-ASSETS.md](LICENSE-ASSETS.md), including restrictions on distribution of derivatives using those assets. Renaming the app does not change those terms.

Windows/Linux 0.4 adds saved chat history, a compact left-aligned model selector and model-aware Ollama performance/reasoning controls. The 8 GB preset is a starting point for 3B/4B models; inspect weight sizes and capabilities before selecting larger models. Integration setup help is available in Settings and the [AI guide](docs/AI-MODELS.md).
