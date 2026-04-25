use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root");

    // Generate icon assets from source image
    generate_icons(workspace_root, &manifest_dir);

    // Embed icon and version info in Windows executables
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "PDF Tools");
        res.set(
            "FileDescription",
            "PDF Tools - Imposition and processing for printers",
        );
        res.compile().expect("Failed to compile Windows resources");
    }

    // Only run for non-WASM targets with pdf-viewer feature
    let target = env::var("TARGET").unwrap();
    if target.contains("wasm32") {
        return;
    }

    // Check if pdf-viewer feature is enabled
    let has_pdf_viewer = env::var("CARGO_FEATURE_PDF_VIEWER").is_ok();
    if !has_pdf_viewer {
        return;
    }

    // Use pdfium_7543 (latest stable as of pdfium-render 0.8.37)
    let pdfium_version = "chromium/7543";

    // Determine platform and architecture
    let (platform, arch, lib_name) = match target.as_str() {
        t if t.contains("apple") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else {
                "x64"
            };
            ("mac", arch, "libpdfium.dylib")
        }
        t if t.contains("linux") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else {
                "x64"
            };
            ("linux", arch, "libpdfium.so")
        }
        t if t.contains("windows") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else if t.contains("i686") {
                "x86"
            } else {
                "x64"
            };
            ("win", arch, "pdfium.dll")
        }
        _ => {
            println!("cargo:warning=Unsupported target platform: {target}");
            return;
        }
    };

    // Set up paths relative to the repository root
    let pdfium_dir = workspace_root.join("vendor").join("pdfium");
    let include_dir = pdfium_dir.join("include");
    // Windows archives put the DLL in bin/ and the import lib (.dll.lib) in lib/
    // On Unix, both the shared library and link target are in lib/
    let dll_dir = pdfium_dir.join(if platform == "win" { "bin" } else { "lib" });
    let link_dir = pdfium_dir.join("lib");
    let lib_path = dll_dir.join(lib_name);

    // Check if already downloaded
    if lib_path.exists() {
        println!(
            "cargo:warning=PDFium already exists at {}",
            lib_path.display()
        );
        fix_library_install_name(&lib_path, platform);
        configure_linking(&link_dir, &include_dir);
        return;
    }

    println!("cargo:warning=Downloading PDFium {pdfium_version} for {platform}-{arch}");

    // Create directories
    fs::create_dir_all(&dll_dir).expect("Failed to create dll directory");
    fs::create_dir_all(&link_dir).expect("Failed to create lib directory");
    fs::create_dir_all(&include_dir).expect("Failed to create include directory");

    // Download URL
    let download_url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/download/{pdfium_version}/pdfium-{platform}-{arch}.tgz"
    );

    // Download and extract
    let temp_file = env::temp_dir().join("pdfium.tgz");

    println!("cargo:warning=Downloading from {download_url}");
    download_file(&download_url, &temp_file);

    let file_size = fs::metadata(&temp_file).map_or(0, |m| m.len());
    println!("cargo:warning=Downloaded {file_size} bytes");

    println!("cargo:warning=Extracting to {}", pdfium_dir.display());
    extract_tarball(&temp_file, &pdfium_dir);

    // Clean up
    let _ = fs::remove_file(&temp_file);

    // Verify installation
    if !lib_path.exists() {
        // List what was actually extracted for diagnostics
        fn list_dir(dir: &Path, prefix: &str) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    println!("cargo:warning={prefix}{}", path.display());
                    if path.is_dir() {
                        list_dir(&path, &format!("{prefix}  "));
                    }
                }
            }
        }
        println!("cargo:warning=Contents of {}:", pdfium_dir.display());
        list_dir(&pdfium_dir, "  ");
        panic!(
            "PDFium installation failed: {} not found",
            lib_path.display()
        );
    }

    println!(
        "cargo:warning=PDFium installed successfully to {}",
        pdfium_dir.display()
    );

    // Fix install name on macOS
    fix_library_install_name(&lib_path, platform);

    configure_linking(&link_dir, &include_dir);
}

fn generate_icons(workspace_root: &Path, manifest_dir: &Path) {
    use image::imageops::FilterType;

    let source = workspace_root.join("res").join("icon.png");
    let assets_dir = manifest_dir.join("assets");

    println!("cargo:rerun-if-changed={}", source.display());

    if !source.exists() {
        println!(
            "cargo:warning=Source icon not found at {}, skipping icon generation",
            source.display()
        );
        return;
    }

    // Skip if source hasn't changed (compare modification times)
    let source_mtime = fs::metadata(&source).and_then(|m| m.modified()).ok();
    let png_output = assets_dir.join("icon-256.png");
    let png_mtime = fs::metadata(&png_output).and_then(|m| m.modified()).ok();
    if let (Some(src), Some(dst)) = (source_mtime, png_mtime)
        && dst >= src
    {
        return;
    }

    println!(
        "cargo:warning=Generating icon assets from {}",
        source.display()
    );
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    let img = image::open(&source).expect("Failed to load source icon");

    // macOS expects ~10% transparent margin so the system can apply its
    // automatic drop-shadow / Liquid Glass treatments and so the icon sits at
    // the same visual size as its neighbors in the Dock and ⌘-Tab switcher.
    // Windows and Linux render icons as-is, so they keep the full-bleed master.
    let padded = pad_for_macos(&img);

    // Full-bleed runtime icon — used by Linux/Windows window icons and copied
    // into the Linux AppImage as the desktop icon.
    img.resize_exact(256, 256, FilterType::Lanczos3)
        .save(&png_output)
        .expect("Failed to save icon-256.png");

    // Padded runtime icon — used by macOS at `cargo run` time so the Dock icon
    // sits at the right visual size during development.
    padded
        .resize_exact(256, 256, FilterType::Lanczos3)
        .save(assets_dir.join("icon-256-mac.png"))
        .expect("Failed to save icon-256-mac.png");

    // Windows ICO (multi-resolution: 16, 32, 48, 256) — full-bleed.
    generate_ico(&img, &assets_dir.join("icon.ico"));

    // macOS ICNS — padded.
    generate_icns(&padded, &assets_dir.join("icon.icns"));

    println!("cargo:warning=Icon assets generated successfully");
}

fn pad_for_macos(img: &image::DynamicImage) -> image::DynamicImage {
    use image::imageops::FilterType;
    use image::{Rgba, RgbaImage};

    let canvas_size = img.width().max(img.height());
    // Apple's icon grid: body fills ~824/1024 ≈ 80.5% of the canvas.
    let body_size = (f64::from(canvas_size) * 0.805).round() as u32;
    let body = img
        .resize_exact(body_size, body_size, FilterType::Lanczos3)
        .to_rgba8();
    let mut canvas = RgbaImage::from_pixel(canvas_size, canvas_size, Rgba([0, 0, 0, 0]));
    let offset = i64::from((canvas_size - body_size) / 2);
    image::imageops::overlay(&mut canvas, &body, offset, offset);
    image::DynamicImage::ImageRgba8(canvas)
}

fn generate_ico(img: &image::DynamicImage, output: &Path) {
    use image::imageops::FilterType;
    use std::io::Write;

    let sizes: &[u32] = &[16, 32, 48, 256];
    let mut png_entries: Vec<Vec<u8>> = Vec::new();

    for &size in sizes {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let mut png_data = Vec::new();
        resized
            .write_to(
                &mut std::io::Cursor::new(&mut png_data),
                image::ImageFormat::Png,
            )
            .expect("Failed to encode PNG for ICO");
        png_entries.push(png_data);
    }

    let mut ico = Vec::new();
    // ICO header: reserved (2), type=1 (2), count (2)
    ico.write_all(&[0, 0]).unwrap(); // reserved
    ico.write_all(&1u16.to_le_bytes()).unwrap(); // type: ICO
    ico.write_all(&(sizes.len() as u16).to_le_bytes()).unwrap();

    // Calculate data offset: header(6) + entries(16 each)
    let mut data_offset: u32 = 6 + (sizes.len() as u32) * 16;

    // Directory entries
    for (i, &size) in sizes.iter().enumerate() {
        let width_byte = if size >= 256 { 0u8 } else { size as u8 };
        ico.push(width_byte); // width
        ico.push(width_byte); // height
        ico.push(0); // color palette count
        ico.push(0); // reserved
        ico.write_all(&1u16.to_le_bytes()).unwrap(); // color planes
        ico.write_all(&32u16.to_le_bytes()).unwrap(); // bits per pixel
        ico.write_all(&(png_entries[i].len() as u32).to_le_bytes())
            .unwrap();
        ico.write_all(&data_offset.to_le_bytes()).unwrap();
        data_offset += png_entries[i].len() as u32;
    }

    // Image data
    for entry in &png_entries {
        ico.write_all(entry).unwrap();
    }

    fs::write(output, &ico).expect("Failed to write icon.ico");
}

fn generate_icns(img: &image::DynamicImage, output: &Path) {
    use icns::{IconFamily, IconType, Image as IcnsImage, PixelFormat};
    use image::imageops::FilterType;

    // TODO: bump the master at res/icon.png to 1024×1024 and add
    // RGBA32_256x256_2x (512) and RGBA32_512x512_2x (1024) entries for sharper
    // rendering on retina Dock and Mission Control.
    let icon_types: &[(u32, IconType)] = &[
        (16, IconType::RGBA32_16x16),
        (32, IconType::RGBA32_32x32),
        (64, IconType::RGBA32_64x64),
        (128, IconType::RGBA32_128x128),
        (256, IconType::RGBA32_256x256),
        (512, IconType::RGBA32_512x512),
    ];

    let mut family = IconFamily::new();

    for &(size, icon_type) in icon_types {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icns_image = IcnsImage::from_data(PixelFormat::RGBA, size, size, rgba.into_raw())
            .unwrap_or_else(|e| panic!("Failed to create ICNS image {size}x{size}: {e}"));
        family
            .add_icon_with_type(&icns_image, icon_type)
            .unwrap_or_else(|e| panic!("Failed to add ICNS icon {size}x{size}: {e}"));
    }

    let file = fs::File::create(output).expect("Failed to create icon.icns");
    family.write(file).expect("Failed to write icon.icns");
}

fn configure_linking(lib_dir: &Path, include_dir: &Path) {
    let target = env::var("TARGET").unwrap();

    // Tell cargo to link against pdfium
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if target.contains("windows") {
        // Windows import lib is named pdfium.dll.lib
        println!("cargo:rustc-link-lib=dylib=pdfium.dll");
    } else {
        println!("cargo:rustc-link-lib=dylib=pdfium");
    }

    // Set rpath so the binary can find the library at runtime
    if target.contains("apple") {
        // macOS: set rpath relative to executable or to the vendor directory
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    } else if target.contains("linux") {
        // Linux: set rpath relative to executable or to the vendor directory
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    }

    // Tell cargo to expose include directory
    println!("cargo:include={}", include_dir.display());

    // Rerun if the library changes
    println!("cargo:rerun-if-changed={}", lib_dir.display());
}

fn download_file(url: &str, dest: &Path) {
    use std::io::Write;

    let response = ureq::get(url)
        .call()
        .unwrap_or_else(|e| panic!("Failed to download {url}: {e}"));

    let mut file = fs::File::create(dest).expect("Failed to create temp file");
    std::io::copy(&mut response.into_reader(), &mut file).expect("Failed to write download");
    file.flush().expect("Failed to flush file");
}

fn extract_tarball(tarball: &Path, dest: &Path) {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let tar_gz = fs::File::open(tarball).expect("Failed to open tarball");
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    for entry in archive.entries().expect("Failed to read tarball entries") {
        let mut entry = entry.expect("Failed to read tarball entry");
        let path = entry
            .path()
            .expect("Failed to read entry path")
            .to_path_buf();
        let entry_type = entry.header().entry_type();

        // Skip symlinks on Windows — the tar crate can silently fail on them
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            println!("cargo:warning=Skipping link entry: {}", path.display());
            continue;
        }

        let out_path = dest.join(&path);
        if entry_type.is_dir() {
            fs::create_dir_all(&out_path).expect("Failed to create directory");
        } else if entry_type.is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create parent directory");
            }
            println!("cargo:warning=  extracting: {}", path.display());
            entry.unpack(&out_path).unwrap_or_else(|e| {
                panic!("Failed to extract {}: {e}", path.display());
            });
        }
    }
}

fn fix_library_install_name(lib_path: &Path, platform: &str) {
    if platform != "mac" {
        return;
    }

    // On macOS, fix the install name to use @rpath
    let output = std::process::Command::new("install_name_tool")
        .arg("-id")
        .arg("@rpath/libpdfium.dylib")
        .arg(lib_path)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            println!(
                "cargo:warning=Fixed install name for {}",
                lib_path.display()
            );
        }
        Ok(result) => {
            println!(
                "cargo:warning=Failed to fix install name: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        Err(e) => {
            println!("cargo:warning=install_name_tool not available: {e}");
        }
    }
}
