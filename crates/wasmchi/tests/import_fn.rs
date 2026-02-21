use std::{fs, process::Command};

#[test]
fn can_import_env_function_and_call_it_from_main() {
    let src = r#"
import fn forty_two(): i32

export fn main(): i32 {
  return forty_two()
}
"#;

    let wasm = wasmchi::compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();

    // E2E via node with a tiny host module.
    let dir = std::env::temp_dir().join("wasmchi-import-e2e");
    let _ = fs::create_dir_all(&dir);

    let wm_path = dir.join("prog.wm");
    fs::write(&wm_path, src).unwrap();

    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        "export default { forty_two: () => 42 }\n",
    )
    .unwrap();

    // Run wasmchi-cli from this repo's debug build (requires it to exist).
    // We'll build it just once in CI; for now, assume cargo test is run after build.
    let exe = std::env::current_dir()
        .unwrap()
        .join("target/debug/wasmchi-cli");

    // If exe doesn't exist (e.g. only testing lib), skip.
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
    assert_eq!(stdout.trim(), "42");
}
