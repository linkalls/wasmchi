use std::{fs, process::Command};

#[test]
fn can_import_env_fn_returning_string_and_print_it() {
    let src = r#"
import fn nanoid(): string

export fn main(): i32 {
  print(nanoid())
  return 0
}
"#;

    let wasm = wasmchi::compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();

    let dir = std::env::temp_dir().join("wasmchi-import-strret-e2e");
    let _ = fs::create_dir_all(&dir);
    let wm_path = dir.join("prog.wm");
    fs::write(&wm_path, src).unwrap();

    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        "export default { nanoid: () => 'abc' }\n",
    )
    .unwrap();

    let exe = std::env::current_dir().unwrap().join("target/debug/wasmchi-cli");
    if !exe.exists() {
        return;
    }

    let out = Command::new(exe)
        .arg("run")
        .arg("--target")
        .arg("node")
        .arg(&wm_path)
        .arg("--host")
        .arg(&host_path)
        .output()
        .expect("wasmchi-cli should run");

    assert!(out.status.success(), "run failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // printed string then return value
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.get(0).copied(), Some("abc"));
    assert_eq!(lines.get(1).copied(), Some("0"));
}
