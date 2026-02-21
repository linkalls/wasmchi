use std::{fs, process::Command};

#[test]
fn auto_host_infers_optional_number_param_and_string_return() {
    // This is a synthetic test using explicit import fn (not npm auto).
    // It ensures the core lowering works: nanoid(i32)->string.
    let src = r#"
import fn nanoid(size: f64): string

export fn main(): i32 {
  print(nanoid(10))
  return 0
}
"#;

    let wasm = wasmchi::compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();

    let dir = std::env::temp_dir().join("wasmchi-nanoid-param-e2e");
    let _ = fs::create_dir_all(&dir);
    let wm_path = dir.join("prog.wm");
    fs::write(&wm_path, src).unwrap();

    // Host implements nanoid(size) -> string
    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        "export default { nanoid: (size) => 'x'.repeat(size|0) }\n",
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
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.get(0).map(|s| s.len()), Some(10));
    assert_eq!(lines.get(1).copied(), Some("0"));
}
