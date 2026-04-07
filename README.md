# 📄 PDF Tools

Collection of PDF processing and generation tools built with Rust.

## ✨ Features

- 🔍 **PDF Viewer** — Page navigation with caching and prefetch
- 🃏 **Flashcard Generator** — Create printable flashcard PDFs from CSV
- 📐 **PDF Imposition** — Signature, perfect, spiral, and case binding with folio/quarto/octavo arrangements, printer's marks, and more

## 📦 Installation

Download the latest release for your platform from the [Releases page](https://github.com/AberrantWolf/pdf-tools/releases).

Extract the archive and keep all files in the same directory — the GUI needs the bundled PDFium library alongside it to run.

| Platform | Prerequisites |
|----------|---------------|
| 🍎 **macOS** | None. If Gatekeeper blocks the app, right-click → "Open" or run `xattr -cr pdf-tools-gui` |
| 🪟 **Windows** | [Visual C++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) (likely already installed) |
| 🐧 **Linux** | A few system libraries — see [INSTALL.md](INSTALL.md) for package names |

> 💡 The `pdft` CLI binary has no extra dependencies and works standalone without PDFium.

## 🚀 Quick Start

```bash
# GUI
./pdf-tools-gui

# CLI help
./pdft --help

# Impose a PDF for signature binding
./pdft impose -i input.pdf -o output.pdf --binding signature --arrangement folio

# Generate flashcards from CSV
./pdft flashcards -i cards.csv -o output.pdf --rows 2 --columns 3
```

<details>
<summary>🔨 Building from source</summary>

```bash
# Build everything (PDFium is downloaded automatically)
cargo build --release

# Run
cargo run --release --bin pdf-tools-gui
cargo run --release --bin pdft -- --help
```

Build without the PDF viewer (if you don't need it or hit PDFium issues):

```bash
cargo build --release --no-default-features
```

See [INSTALL.md](INSTALL.md) for troubleshooting build issues.

</details>

<details>
<summary>📋 Releasing a new version</summary>

1. Update the version in `Cargo.toml` → `[workspace.package]`
2. Commit and push to main
3. Tag and push:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. The [Release workflow](.github/workflows/release.yml) automatically builds for Linux (x86_64), macOS (x86_64 + ARM), and Windows (x86_64), then creates a GitHub Release with all archives attached.

</details>

## 🏗️ Architecture

```
pdf-tools/
├── crates/
│   ├── 🖥️ pdf-tools-cli      CLI (binary: pdft)
│   ├── 🪟 pdf-tools-gui      Desktop GUI (egui) + WASM web
│   ├── 📐 pdf-impose          Imposition library
│   ├── 🃏 pdf-flashcards      Flashcard generation
│   └── ⚡ pdf-async-runtime   Async command/update channels
```

## 📄 License

MIT
