# Source provenance

Repository: [ZuanCrisp/MoMo](https://github.com/ZuanCrisp/MoMo).

## Upstream

The source was imported from the `coucou-0.1.2.zip` archive of [Louis-CFM/coucou](https://github.com/Louis-CFM/coucou). The macOS source is from version 0.1.2; the Windows/Linux workspace declares version 0.1.1.

The original source copyright and MIT license are retained in `LICENSE`. The original `LICENSE-ASSETS.md` is also retained unchanged. Upstream links remain where they provide attribution or reference original material.

## MoMo changes

- Application names, interface labels, package names, hook names and relevant runtime identifiers use MoMo.
- Windows uses the application identifier `app.momo.desktop`.
- Internal macOS NotchBuddy project names and its primary bundle identifier are retained for compatibility.
- Windows packaging includes both license notices and supports an offline WebView2 installer.
- The Windows hook relay uses the static MSVC runtime to avoid a separate Visual C++ runtime dependency.
- A local toolchain launcher, portable build documentation and Windows build validation are included.
- Rust source is formatted with Rustfmt.
- Windows/Linux version 0.2.0 adds multi-provider AI profiles, local model connections, ordered key/model fallback, credential migration and a new settings/chat interface with protocol and UI tests.
- Windows/Linux version 0.3.0 adds themed dropdowns, fixes island clipping, removes hook installation UI, provides Ollama library setup and adds bounded AI desktop tools for opening apps and creating notes. Model files remain outside Git and distribution packages.

Original character artwork, icons, animations, sounds and demo media remain upstream assets governed by their separate license. The rebrand does not grant additional rights to them.

The repository starts with a source import rather than importing upstream Git history. Original authorship is preserved through these credits and license notices.
