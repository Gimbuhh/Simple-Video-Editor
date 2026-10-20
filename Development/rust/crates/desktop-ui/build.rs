fn main() {
    use std::{path::PathBuf, process::Command};
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let tools = development.join(".tools/llvm");
    println!(
        "cargo:rustc-link-search=native={}",
        tools.join("lib").display()
    );
    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let version = env!("CARGO_PKG_VERSION");
    let numeric = version.replace('.', ",") + ",0";
    let icon = development
        .join("assets/app.ico")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let manifest = development
        .join("assets/app.manifest")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let rc = format!(
        r#"1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "Simple Video Editor"
   VALUE "FileVersion", "{version}.0"
   VALUE "ProductName", "Simple Video Editor"
   VALUE "ProductVersion", "{version}"
   VALUE "OriginalFilename", "Simple Video Editor.exe"
   VALUE "LegalCopyright", "Copyright Gimbuhh. MIT licensed."
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
"#
    );
    let source = output.join("editor.rc");
    let resource = output.join("editor.res");
    let object = output.join("editor-resource.obj");
    std::fs::write(&source, rc).unwrap();
    assert!(
        Command::new(tools.join("bin/llvm-rc.exe"))
            .arg("-no-preprocess")
            .arg("/fo")
            .arg(&resource)
            .arg(&source)
            .status()
            .unwrap()
            .success(),
        "Windows resource compilation failed"
    );
    assert!(
        Command::new(tools.join("bin/llvm-cvtres.exe"))
            .arg("/MACHINE:X64")
            .arg(format!("/OUT:{}", object.display()))
            .arg(&resource)
            .status()
            .unwrap()
            .success(),
        "Windows resource conversion failed"
    );
    println!("cargo:rustc-link-arg={}", object.display());
    println!(
        "cargo:rerun-if-changed={}",
        development.join("assets/app.ico").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        development.join("assets/app.manifest").display()
    );
}
