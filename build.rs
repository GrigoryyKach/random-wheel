// Вшивает в RandomWheel.exe иконку (assets/icon.ico, рисуется nimvoro/tools/make-icons.js) и сведения о версии,
// чтобы в Проводнике и на панели задач было «Random Wheel» со своим значком.
// MSVC-тулчейн: нужен rc.exe из Windows SDK, GNU-тулчейн: windres из MinGW.
// Если инструмента нет — сборка продолжается без ресурсов.

use std::path::{Path, PathBuf};

fn find_rc() -> Option<PathBuf> {
    if let Some(rc) = std::env::var_os("RC") {
        return Some(rc.into());
    }
    let arch = if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "arm64" } else { "x64" };
    for base in [r"C:\Program Files (x86)\Windows Kits\10\bin", r"C:\Program Files\Windows Kits\10\bin"] {
        let Ok(rd) = std::fs::read_dir(base) else { continue };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs.into_iter().rev() {
            let rc = d.join(arch).join("rc.exe");
            if rc.exists() {
                return Some(rc);
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("icon.ico");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let mut nums: Vec<u32> = version.split('.').filter_map(|p| p.parse().ok()).collect();
    nums.resize(4, 0);
    let commas = nums.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let rc_text = format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041904B0"
    BEGIN
      VALUE "FileDescription", "Random Wheel"
      VALUE "ProductName", "Random Wheel"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "RandomWheel.exe"
      VALUE "InternalName", "RandomWheel"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x419, 1200
  END
END
"#,
        ico = ico.display().to_string().replace('\\', "\\\\"),
    );
    let rc_path = out.join("app.rc");
    std::fs::write(&rc_path, rc_text).unwrap();
    if msvc {
        let res = out.join("app.res");
        let Some(rc) = find_rc() else {
            println!("cargo:warning=rc.exe не найден — exe соберётся без иконки и сведений о версии");
            return;
        };
        let ok = std::process::Command::new(&rc).args(["/nologo", "/fo"]).arg(&res).arg(&rc_path).status().is_ok_and(|s| s.success());
        if ok && Path::new(&res).exists() {
            println!("cargo:rustc-link-arg-bins={}", res.display());
        } else {
            println!("cargo:warning=rc.exe не смог собрать ресурсы — exe будет без иконки");
        }
    } else {
        let obj = out.join("app_res.o");
        let windres = std::env::var("WINDRES").unwrap_or_else(|_| "windres".into());
        let ok = std::process::Command::new(&windres)
            .arg("--input")
            .arg(&rc_path)
            .arg("--output")
            .arg(&obj)
            .args(["--input-format=rc", "--output-format=coff"])
            .status()
            .is_ok_and(|s| s.success());
        if ok && Path::new(&obj).exists() {
            println!("cargo:rustc-link-arg-bins={}", obj.display());
        } else {
            println!("cargo:warning=windres не найден или не сработал — exe соберётся без иконки и сведений о версии");
        }
    }
}
