#[cfg(not(feature = "dox"))]
fn main() -> anyhow::Result<()> {
    use download_cef::{CefIndex, OsAndArch};
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    println!("cargo::rerun-if-changed=build.rs");

    let target = env::var("TARGET")?;
    let os_arch = OsAndArch::try_from(target.as_str())?;

    println!("cargo::rerun-if-env-changed=FLATPAK");
    println!("cargo::rerun-if-env-changed=CEF_PATH");
    println!("cargo::rerun-if-env-changed=CEF_ARCHIVE_URL");
    let package_version = env::var("CARGO_PKG_VERSION")?;
    let cef_version = download_cef::default_version(&package_version);

    let check_archive = |path: &Path| -> anyhow::Result<()> {
        download_cef::check_archive_json(&package_version, &path.to_string_lossy())?;
        Ok(())
    };

    let resolve_cef_dir = |location: &Path| -> anyhow::Result<PathBuf> {
        let cef_dir = location.join(os_arch.to_string());

        if !fs::exists(&cef_dir)? {
            let download_url = download_cef::default_download_url();
            let index = CefIndex::download_from(&download_url)?;
            let platform = index.platform(&target)?;
            let version = platform.version(&cef_version)?;

            let archive = version.download_archive_from(&download_url, location, false)?;
            let extracted_dir =
                download_cef::extract_target_archive(&target, &archive, location, false)?;
            let extracted_dir_canonical = fs::canonicalize(&extracted_dir)?;
            let cef_dir_canonical = fs::canonicalize(&cef_dir)?;
            if extracted_dir_canonical != cef_dir_canonical {
                return Err(anyhow::anyhow!(
                    "extracted dir {extracted_dir_canonical:?} does not match cef_dir {cef_dir_canonical:?}",
                ));
            }

            version.write_archive_json(extracted_dir)?;
        }

        Ok(cef_dir)
    };

    let resolve_from_versioned = |configured_path: &Path| -> anyhow::Result<PathBuf> {
        let versioned_location = configured_path.join(&cef_version);
        let resolved = resolve_cef_dir(&versioned_location)?;
        println!(
            "Using versioned CEF path from environment: {}",
            resolved.display()
        );
        check_archive(&resolved)?;
        Ok(resolved)
    };

    let download_to_versioned = |configured_path: &Path, reason: &str| -> anyhow::Result<PathBuf> {
        let versioned_location = configured_path.join(&cef_version);
        println!(
            "{reason}, downloading archive to: {}",
            versioned_location.display()
        );
        let resolved = resolve_cef_dir(&versioned_location)?;
        println!("Using downloaded CEF path: {}", resolved.display());
        Ok(resolved)
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    const DEFAULT_LINUX_X64_ARCHIVE_URL: &str =
        "https://cloud.frozenblock.net/s/82X5EQQ2ZoiPLWs/download";
    let default_archive_url = (target == "x86_64-unknown-linux-gnu")
        .then(|| DEFAULT_LINUX_X64_ARCHIVE_URL.to_string());

    let cef_dir = if env::var("FLATPAK").is_ok() {
        let cef_path = String::from("/usr/lib");
        println!("Using CEF path from FLATPAK: {cef_path}");
        let cef_path = PathBuf::from(cef_path);
        check_archive(&cef_path)?;
        cef_path
    } else if let Ok(cef_path) = env::var("CEF_PATH") {
        let configured_path = PathBuf::from(cef_path);
        if fs::exists(&configured_path)? {
            let versioned_location = configured_path.join(&cef_version);
            if fs::exists(&versioned_location)? {
                resolve_from_versioned(&configured_path)?
            } else {
                println!(
                    "Using CEF path from environment: {}",
                    configured_path.display()
                );
                match check_archive(&configured_path) {
                    Ok(()) => configured_path,
                    Err(error) => download_to_versioned(
                        &configured_path,
                        &format!("CEF_PATH is invalid ({error})"),
                    )?,
                }
            }
        } else {
            download_to_versioned(&configured_path, "CEF_PATH does not exist")?
        }
    } else if let Some(archive_url) = env::var("CEF_ARCHIVE_URL").ok().or(default_archive_url) {
        let versioned_location = out_dir.join(&cef_version);
        let cef_dir = versioned_location.join(os_arch.to_string());
        if fs::exists(&cef_dir)? && check_archive(&cef_dir).is_ok() {
            println!("Using cached CEF archive from: {}", cef_dir.display());
            cef_dir
        } else {
            download_and_extract_archive(&archive_url, &target, &versioned_location)?
        }
    } else {
        resolve_cef_dir(&out_dir)?
    };

    // TODO: far from ideal, but there's no other way to get the target dir, see <https://github.com/rust-lang/cargo/issues/9661>
    let target_dir = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let cef_dir_str = cef_dir.to_string_lossy().into_owned();

    // Re-run when the resolved CEF directory changes/deletes.
    println!("cargo::rerun-if-changed={cef_dir_str}");

    println!("cargo::metadata=CEF_DIR={cef_dir_str}");
    println!("cargo::rustc-link-search=native={cef_dir_str}");

    let mut cef_dll_wrapper = cmake::Config::new(&cef_dir);
    cef_dll_wrapper
        .generator("Ninja")
        .profile("RelWithDebInfo")
        .build_target("libcef_dll_wrapper");

    let project_arch = match os_arch.arch {
        "aarch64" => "arm64",
        arch => arch,
    };

    let sandbox = if cfg!(feature = "sandbox") {
        "ON"
    } else {
        "OFF"
    };

    match os_arch.os {
        "linux" => {
            // On Windows and Linux the cef files usually have to be next to the main binary.
            // On macOS it's more complicated so we'll leave it to tools like tauri-cli for now.
            copy_cef_runtime_files(&cef_dir, target_dir)?;

            println!("cargo::rustc-link-lib=dylib=cef");
        }
        "windows" => {
            // On Windows and Linux the cef files usually have to be next to the main binary.
            // On macOS it's more complicated so we'll leave it to tools like tauri-cli for now.
            copy_cef_runtime_files(&cef_dir, target_dir)?;

            let sdk_libs = [
                "comctl32.lib",
                "delayimp.lib",
                "mincore.lib",
                "powrprof.lib",
                "propsys.lib",
                "runtimeobject.lib",
                "setupapi.lib",
                "shcore.lib",
                "shell32.lib",
                "shlwapi.lib",
                "user32.lib",
                "version.lib",
                "wbemuuid.lib",
                "winmm.lib",
            ]
            .join(" ");

            let build_dir = cef_dll_wrapper
                .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded")
                .define("CMAKE_OBJECT_PATH_MAX", "500")
                .define("CMAKE_STATIC_LINKER_FLAGS", &sdk_libs)
                .define("PROJECT_ARCH", project_arch)
                .define("USE_SANDBOX", sandbox)
                .build()
                .to_string_lossy()
                .into_owned();

            println!("cargo::rustc-link-search=native={build_dir}/build/libcef_dll_wrapper");
            println!("cargo::rustc-link-lib=static=libcef_dll_wrapper");

            println!("cargo::rustc-link-lib=dylib=libcef");
        }
        "macos" => {
            println!("cargo::rustc-link-lib=framework=AppKit");

            let build_dir = cef_dll_wrapper
                .no_default_flags(true)
                .define("PROJECT_ARCH", project_arch)
                .define("USE_SANDBOX", sandbox)
                .build()
                .to_string_lossy()
                .into_owned();
            println!("cargo::rustc-link-search=native={build_dir}/build/libcef_dll_wrapper");
            println!("cargo::rustc-link-lib=static=cef_dll_wrapper");
        }
        os => unimplemented!("unknown target {os}"),
    }

    Ok(())
}

#[cfg(not(feature = "dox"))]
fn copy_directory(src: &std::path::Path, dest: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dest.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(not(feature = "dox"))]
fn copy_cef_runtime_files(
    cef_dir: &std::path::Path,
    target_dir: &std::path::Path,
) -> Result<(), std::io::Error> {
    copy_directory(cef_dir, target_dir)?;

    const LOCALES_DIR: &str = "locales";
    copy_directory(&cef_dir.join(LOCALES_DIR), &target_dir.join(LOCALES_DIR))?;

    Ok(())
}

/// Fetches a prebuilt CEF distribution from an arbitrary URL instead of the
/// official versioned CDN index. The URL must point directly at a
/// `.tar.zst` archive containing a single top-level directory with the
/// standard CEF distribution layout (`CMakeLists.txt`, `include/`,
/// `libcef_dll/`, `Release/`, `Resources/`, ...).
#[cfg(not(feature = "dox"))]
fn download_and_extract_archive(
    url: &str,
    target: &str,
    versioned_location: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    use download_cef::OsAndArch;
    use std::fs;

    fs::create_dir_all(versioned_location)?;
    let os_arch = OsAndArch::try_from(target)?;
    let cef_dir = versioned_location.join(os_arch.to_string());

    println!("Downloading CEF archive from: {url}");
    let archive_path = versioned_location.join("cef_archive.tar.zst");
    // Shells out to curl rather than using ureq: some hosts (observed with a
    // Nextcloud share) send a chunked-encoding response that ureq's HTTP/1.1
    // parser rejects ("chunk length cannot be read as a number") but curl
    // (negotiating HTTP/2) handles fine.
    let status = std::process::Command::new("curl")
        .args(["-fL", "--retry", "3", "-o"])
        .arg(&archive_path)
        .arg(url)
        .status()?;
    if !status.success() {
        anyhow::bail!("curl download of {url} failed with status {status}");
    }

    let extract_dir = versioned_location.join("cef_archive_extracted");
    let _ = fs::remove_dir_all(&extract_dir);
    fs::create_dir_all(&extract_dir)?;

    println!("Extracting CEF archive to: {}", extract_dir.display());
    let status = std::process::Command::new("tar")
        .arg("--zstd")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&extract_dir)
        .status()?;
    if !status.success() {
        anyhow::bail!("tar extraction failed with status {status}");
    }
    fs::remove_file(&archive_path)?;

    // The archive holds a single top-level directory; that becomes cef_dir.
    let mut entries = fs::read_dir(&extract_dir)?;
    let top_level = entries
        .next()
        .ok_or_else(|| anyhow::anyhow!("archive at {url} was empty"))??
        .path();
    if entries.next().is_some() {
        anyhow::bail!("expected a single top-level directory in the archive from {url}");
    }

    if cef_dir.exists() {
        fs::remove_dir_all(&cef_dir)?;
    }
    fs::rename(&top_level, &cef_dir)?;
    fs::remove_dir_all(&extract_dir)?;

    let cef_file = download_cef::CefFile {
        file_type: "standard".to_string(),
        name: format!(
            "cef_binary_{}+custom_{}",
            download_cef::default_version(&std::env::var("CARGO_PKG_VERSION")?),
            os_arch
        ),
        sha1: String::new(),
    };
    cef_file.write_archive_json(&cef_dir)?;

    Ok(cef_dir)
}

#[cfg(feature = "dox")]
fn main() {}
