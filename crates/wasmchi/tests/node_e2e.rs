use std::{fs, process::Command};

#[test]
fn node_e2e_runs_main_and_prints_return_value() {
    // Build a tiny program
    let src = "export fn main(): i32 {\n  print(\"h\" + \"i\")\n  let x = 40\n  return x + 2\n}\n";
    let wasm = wasmchi::compile_to_wasm(src).unwrap();

    // Write wasm + host JS to temp dir
    let dir = std::env::temp_dir().join("wasmchi-e2e");
    let _ = fs::create_dir_all(&dir);

    let wasm_path = dir.join("out.wasm");
    fs::write(&wasm_path, wasm).unwrap();

    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        r#"import fs from 'node:fs/promises';

const wasmPath = process.argv[2];
const wasmBytes = await fs.readFile(wasmPath);

let bytes;
let decoder;
function print(ptr, len) {
  const s = decoder.decode(bytes.subarray(ptr, ptr + len));
  process.stdout.write(s + "\n");
}

const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { print } });
const memory = instance.exports.memory;
bytes = new Uint8Array(memory.buffer);
decoder = new TextDecoder('utf-8');

const ret = instance.exports.main();
console.log(ret);
"#,
    )
    .unwrap();

    // Run node
    let out = Command::new("node")
        .arg(&host_path)
        .arg(&wasm_path)
        .output()
        .expect("node should run");

    assert!(out.status.success(), "node failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // output should contain printed line + return value line
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.get(0).copied(), Some("hi"));
    assert_eq!(lines.get(1).copied(), Some("42"));
}
