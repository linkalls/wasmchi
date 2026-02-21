use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        usage_and_exit();
        return;
    };

    match cmd.as_str() {
        "build" => {
            let Some(input) = args.next() else { usage_and_exit(); return; };
            let mut out: Option<String> = None;
            while let Some(a) = args.next() {
                if a == "--out" {
                    out = args.next();
                } else {
                    eprintln!("unknown arg: {a}");
                    process::exit(2);
                }
            }

            let input_path = PathBuf::from(input);
            let out_path = out
                .map(PathBuf::from)
                .unwrap_or_else(|| input_path.with_extension("wasm"));

            let wasm = compile_file(&input_path);

            fs::write(&out_path, wasm).unwrap_or_else(|e| {
                eprintln!("failed to write {}: {e}", out_path.display());
                process::exit(1);
            });

            eprintln!("built {}", out_path.display());
        }
        "run" => {
            let target = args.next().unwrap_or_else(|| "node".to_string());
            if target != "--target" {
                // allow: wasmchi run --target node file.wm
                // but also allow: wasmchi run file.wm (defaults to node)
                let input = target;
                run_node(PathBuf::from(input));
                return;
            }
            let tgt = args.next().unwrap_or_else(|| "node".to_string());
            let Some(input) = args.next() else { usage_and_exit(); return; };
            match tgt.as_str() {
                "node" => run_node(PathBuf::from(input)),
                "browser" => {
                    let out_dir = PathBuf::from("dist");
                    bundle_browser(PathBuf::from(input), out_dir);
                }
                _ => {
                    eprintln!("unknown target: {tgt}");
                    process::exit(2);
                }
            }
        }
        "bundle" => {
            let target = args.next().unwrap_or_else(|| "--target".to_string());
            if target != "--target" {
                usage_and_exit();
            }
            let tgt = args.next().unwrap_or_else(|| "browser".to_string());
            let Some(input) = args.next() else { usage_and_exit(); return; };

            let mut out_dir: PathBuf = PathBuf::from("dist");
            while let Some(a) = args.next() {
                if a == "--out-dir" {
                    if let Some(d) = args.next() {
                        out_dir = PathBuf::from(d);
                    }
                } else {
                    eprintln!("unknown arg: {a}");
                    process::exit(2);
                }
            }

            match tgt.as_str() {
                "browser" => bundle_browser(PathBuf::from(input), out_dir),
                _ => {
                    eprintln!("only --target browser supported for bundle right now");
                    process::exit(2);
                }
            }
        }
        _ => usage_and_exit(),
    }
}

fn compile_file(path: &PathBuf) -> Vec<u8> {
    let src = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("failed to read {}: {e}", path.display());
        process::exit(1);
    });

    wasmchi::compile_to_wasm(&src).unwrap_or_else(|e| {
        eprintln!("compile error: {e}");
        process::exit(1);
    })
}

fn run_node(input_path: PathBuf) {
    let wasm = compile_file(&input_path);

    let tmp_dir = std::env::temp_dir().join("wasmchi");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let wasm_path = tmp_dir.join("out.wasm");
    std::fs::write(&wasm_path, wasm).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", wasm_path.display());
        process::exit(1);
    });

    let host_path = tmp_dir.join("host.mjs");
    std::fs::write(&host_path, node_host_mjs()).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", host_path.display());
        process::exit(1);
    });

    let status = std::process::Command::new("node")
        .arg(&host_path)
        .arg(&wasm_path)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run node: {e}");
            process::exit(1);
        });

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn bundle_browser(input_path: PathBuf, out_dir: PathBuf) {
    let wasm = compile_file(&input_path);

    let _ = std::fs::create_dir_all(&out_dir);

    let wasm_path = out_dir.join("out.wasm");
    std::fs::write(&wasm_path, wasm).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", wasm_path.display());
        process::exit(1);
    });

    let app_path = out_dir.join("app.mjs");
    std::fs::write(&app_path, browser_app_mjs()).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", app_path.display());
        process::exit(1);
    });

    let html_path = out_dir.join("index.html");
    std::fs::write(
        &html_path,
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>wasmchi</title>\n<script type=\"module\" src=\"./app.mjs\"></script>\n",
    )
    .unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", html_path.display());
        process::exit(1);
    });

    eprintln!("bundled browser app in {}", out_dir.display());
    eprintln!("serve it (required for fetch):");
    eprintln!("  cd {} && python3 -m http.server 8000", out_dir.display());
    eprintln!("then open:");
    eprintln!("  http://localhost:8000/");
}

fn node_host_mjs() -> &'static str {
    r#"import fs from 'node:fs/promises';

const wasmPath = process.argv[2];
if (!wasmPath) {
  console.error('usage: node host.mjs <file.wasm>');
  process.exit(2);
}
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
"#
}

fn browser_app_mjs() -> &'static str {
    r#"const wasmBytes = await (await fetch('./out.wasm')).arrayBuffer();

let bytes;
let decoder;
function print(ptr, len) {
  const s = decoder.decode(bytes.subarray(ptr, ptr + len));
  console.log(s);
}

const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { print } });
const memory = instance.exports.memory;
bytes = new Uint8Array(memory.buffer);
decoder = new TextDecoder('utf-8');

const ret = instance.exports.main();
console.log(ret);
"#
}

fn usage_and_exit() {
    eprintln!(
        "wasmchi (wip)\n\nUSAGE:\n  wasmchi build <file.wm> [--out <out.wasm>]\n  wasmchi run [--target node|browser] <file.wm>\n  wasmchi bundle --target browser <file.wm> [--out-dir <dir>]\n",
    );
    process::exit(2);
}
