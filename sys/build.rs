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
    let package_version = env::var("CARGO_PKG_VERSION")?;
    let cef_version = download_cef::default_version(&package_version);

    let check_archive = |path: &Path| -> anyhow::Result<()> {
        download_cef::check_archive_json(&package_version, &path.display().to_string())?;
        Ok(())
    };

    let resolve_cef_dir = |location: &Path| -> anyhow::Result<PathBuf> {
        let cef_dir = location.join(os_arch.to_string());

        if !fs::exists(&cef_dir)? {
            let index = CefIndex::download()?;
            let platform = index.platform(&target)?;
            let version = platform.version(&cef_version)?;

            let archive = version.download_archive(location, false)?;
            let extracted_dir = download_cef::extract_target_archive(&target, &archive, location, false)?;
            if extracted_dir != cef_dir {
                return Err(anyhow::anyhow!(
                    "extracted dir {extracted_dir:?} does not match cef_dir {cef_dir:?}",
                ));
            }

            version.write_archive_json(extracted_dir)?;
        }

        Ok(cef_dir)
    };

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
                let resolved = resolve_cef_dir(&versioned_location)?;
                println!(
                    "Using versioned CEF path from environment: {}",
                    resolved.display()
                );
                check_archive(&resolved)?;
                resolved
            } else {
                println!("Using CEF path from environment: {}", configured_path.display());
                check_archive(&configured_path)?;
                configured_path
            }
        } else {
            let versioned_location = configured_path.join(&cef_version);
            println!(
                "CEF_PATH does not exist, downloading archive to: {}",
                versioned_location.display()
            );
            let resolved = resolve_cef_dir(&versioned_location)?;
            println!("Using downloaded CEF path: {}", resolved.display());
            resolved
        }
    } else {
        let out_dir = PathBuf::from(env::var("OUT_DIR")?);
        resolve_cef_dir(&out_dir)?
    };

    let cef_dir = cef_dir.display().to_string();

    println!("cargo::metadata=CEF_DIR={cef_dir}");
    println!("cargo::rustc-link-search=native={cef_dir}");

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
            println!("cargo::rustc-link-lib=dylib=cef");
        }
        "windows" => {
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
                .display()
                .to_string();

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
                .display()
                .to_string();
            println!("cargo::rustc-link-search=native={build_dir}/build/libcef_dll_wrapper");
            println!("cargo::rustc-link-lib=static=cef_dll_wrapper");
        }
        os => unimplemented!("unknown target {os}"),
    }

    Ok(())
}

#[cfg(feature = "dox")]
fn main() {}
