use std::{fs, process::Command};

#[test]
fn can_import_env_log_string_literal() {
    let src = r#"
import fn js_log(msg: string): void

export fn main(): i32 {
  js_log("hello")
  return 0
}
"#;

    let wasm = wasmchi::compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();

    let dir = std::env::temp_dir().join("wasmchi-import-string-e2e");
    let _ = fs::create_dir_all(&dir);
    let wm_path = dir.join("prog.wm");
    fs::write(&wm_path, src).unwrap();

    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        r#"export default {
  js_log: (ptr, len) => {
    // wasm exports memory; host glue will already pass env.print, but here we decode ourselves
    // by reading wasm memory in the generated host. For now we just print ptr/len.
    // (The wasmchi-generated host will still provide its own print.)
    // We'll keep this function signature stable.
    return 0;
  }
}
"#,
    )
    .unwrap();

    let exe = std::env::current_dir().unwrap().join("target/debug/wasmchi-cli");
    if !exe.exists() {
        return;
    }

    // Run: expect it to succeed (we're not asserting output yet).
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
}
