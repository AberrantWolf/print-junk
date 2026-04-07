# 📦 Installation Guide

## From a release

Download the archive for your platform from the [Releases page](https://github.com/AberrantWolf/pdf-tools/releases) and extract it. Keep all files in the same directory.

### 🐧 Linux dependencies

The GUI requires a few system libraries for graphics and file dialogs:

```bash
# Debian/Ubuntu
sudo apt install libxcb-render0 libxcb-shape0 libxcb-xfixes0 libxkbcommon0 libgl1 libgtk-3-0

# Fedora
sudo dnf install libxcb libxkbcommon mesa-libGL gtk3

# Arch
sudo pacman -S libxcb libxkbcommon mesa gtk3
```

### 🍎 macOS Gatekeeper

Since the app isn't signed, macOS may block it. To allow it:

- Right-click the app → "Open", or
- Run `xattr -cr pdf-tools-gui` in the extracted directory

### 🪟 Windows

You may need the [Visual C++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) if it isn't already installed.

## From source

### Supported platforms

| Platform | Architectures |
|----------|---------------|
| 🍎 macOS | Intel (x64), Apple Silicon (arm64) |
| 🐧 Linux | x64, arm64 |
| 🪟 Windows | x64, x86, arm64 |

### Building

```bash
cargo build --release
```

The build script automatically downloads [PDFium](https://github.com/bblanchon/pdfium-binaries) (chromium/7543) to `vendor/pdfium/` and configures linking. No manual setup needed.

To build without the PDF viewer:

```bash
cargo build --release --no-default-features
```

### Linux build dependencies

In addition to the runtime libraries above, you need development headers:

```bash
# Debian/Ubuntu
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libgl1-mesa-dev libgtk-3-dev libatk1.0-dev
```

## Troubleshooting

### PDFium download fails during build

1. Check your internet connection
2. Try building again — the download is cached after the first success
3. As a last resort, manually download the correct archive from [pdfium-binaries releases](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7543) and extract it to `vendor/pdfium/` in the repository root

### "Failed to load PDFium" at runtime

Make sure the PDFium library file (`libpdfium.dylib`, `libpdfium.so`, or `pdfium.dll`) is in the same directory as the `pdf-tools-gui` binary.

### Force re-download of PDFium

```bash
rm -rf vendor/pdfium
cargo clean -p pdf-tools-gui
cargo build --release
```

### Getting help

Open an issue with your OS, architecture (`uname -m`), and the full error message.
