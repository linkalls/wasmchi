use std::{env, fs, path::PathBuf, process};

fn import_meta_from_source(src: &str) -> String {
    let Ok(program) = wasmchi::parse(src) else {
        return "[]".to_string();
    };

    // JSON array: [{ name: "js_log", params: ["string","i32"], ret: "void" }]
    let mut items: Vec<String> = Vec::new();
    for item in program.items {
        if let wasmchi::Item::ImportFn(im) = item {
            let params: Vec<&'static str> = im
                .params
                .iter()
                .map(|p| match p.ty {
                    wasmchi::Type::I32 => "i32",
                    wasmchi::Type::String => "string",
                    wasmchi::Type::Void => "void",
                })
                .collect();
            let ret: &'static str = match im.ret_ty {
                wasmchi::Type::I32 => "i32",
                wasmchi::Type::String => "string",
                wasmchi::Type::Void => "void",
            };
            items.push(format!(
                "{{\"name\":{},\"params\":[{}],\"ret\":{}}}",
                json_str(&im.name),
                params.into_iter().map(json_str).collect::<Vec<_>>().join(","),
                json_str(ret)
            ));
        }
    }

    format!("[{}]", items.join(","))
}

fn json_str(s: impl AsRef<str>) -> String {
    // minimal JSON string escaper
    let s = s.as_ref();
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

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

            let (_src, wasm) = compile_file(&input_path);

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
                run_node(PathBuf::from(input), None);
                return;
            }
            let tgt = args.next().unwrap_or_else(|| "node".to_string());
            let Some(input) = args.next() else { usage_and_exit(); return; };
            match tgt.as_str() {
                "node" => {
                    // optional: --host <path-to-host-module>
                    let mut host: Option<PathBuf> = None;
                    while let Some(a) = args.next() {
                        if a == "--host" {
                            host = args.next().map(PathBuf::from);
                        } else {
                            eprintln!("unknown arg: {a}");
                            process::exit(2);
                        }
                    }
                    run_node(PathBuf::from(input), host);
                }
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

fn compile_file(path: &PathBuf) -> (String, Vec<u8>) {
    let src = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("failed to read {}: {e}", path.display());
        process::exit(1);
    });

    let wasm = wasmchi::compile_to_wasm(&src).unwrap_or_else(|e| {
        eprintln!("compile error: {e}");
        process::exit(1);
    });

    (src, wasm)
}

fn run_node(input_path: PathBuf, host_module: Option<PathBuf>) {
    let (src, wasm) = compile_file(&input_path);

    let host_module = host_module.and_then(|p| p.canonicalize().ok());
    let import_meta = import_meta_from_source(&src);

    let tmp_dir = std::env::temp_dir().join("wasmchi");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let wasm_path = tmp_dir.join("out.wasm");
    std::fs::write(&wasm_path, wasm).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", wasm_path.display());
        process::exit(1);
    });

    let host_path = tmp_dir.join("host.mjs");
    let host_src = node_host_mjs(host_module.as_deref(), &import_meta);
    std::fs::write(&host_path, host_src).unwrap_or_else(|e| {
        eprintln!("failed to write {}: {e}", host_path.display());
        process::exit(1);
    });

    let cwd = input_path.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

    let status = std::process::Command::new("node")
        .current_dir(&cwd)
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
    let (_src, wasm) = compile_file(&input_path);

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

fn node_host_mjs(host_module: Option<&std::path::Path>, import_meta_json: &str) -> String {
    // We generate a host that can optionally `import(env)` from a user-provided module.
    // The user module should default-export an object of functions, e.g.
    //   export default { now_i32: () => (Date.now()|0) }
    // Those functions are exposed under `env.*` imports.

    let env_import = if let Some(p) = host_module {
        format!(
            "import {{ pathToFileURL }} from 'node:url';\nconst userEnvMod = await import(pathToFileURL({:?}).href);\nconst userEnv = userEnvMod.default ?? userEnvMod.env ?? {{}};\n",
            p.to_string_lossy().to_string()
        )
    } else {
        "const userEnv = {};\n".to_string()
    };

    format!(
        r#"import fs from 'node:fs/promises';
{env_import}

const WASMCHI_IMPORTS = {import_meta_json};

const wasmPath = process.argv[2];
if (!wasmPath) {{
  console.error('usage: node host.mjs <file.wasm>');
  process.exit(2);
}}
const wasmBytes = await fs.readFile(wasmPath);

let bytes;
let decoder;
const encoder = new TextEncoder();
let allocFn;
function alloc(len) {{
  return (allocFn(len) | 0);
}}
function readString(ptr, len) {{
  return decoder.decode(bytes.subarray(ptr, ptr + len));
}}

function print(ptr, len) {{
  process.stdout.write(readString(ptr, len) + "\n");
}}

function wrapUserEnv(userEnv) {{
  const wrapped = {{ ...userEnv }};

  for (const spec of WASMCHI_IMPORTS) {{
    const fn = userEnv[spec.name];
    if (typeof fn !== 'function') continue;

    // Wrapper matches wasm-lowered ABI (string => ptr,len)
    wrapped[spec.name] = (...loweredArgs) => {{
      const args = [];
      let i = 0;
      for (const t of spec.params) {{
        if (t === 'i32') {{
          args.push(loweredArgs[i]);
          i += 1;
        }} else if (t === 'string') {{
          const ptr = loweredArgs[i];
          const len = loweredArgs[i+1];
          args.push(readString(ptr, len));
          i += 2;
        }} else {{
          throw new Error('unsupported param type: ' + t);
        }}
      }}

      const ret = fn(...args);

      if (spec.ret === 'void') {{
        return;
      }}
      if (spec.ret === 'i32') {{
        return ret | 0;
      }}
      if (spec.ret === 'string') {{
        const s = String(ret ?? "");
        const utf8 = encoder.encode(s);
        const ptr = alloc(utf8.length);
        bytes.set(utf8, ptr);
        return [ptr, utf8.length];
      }}

      throw new Error('unsupported return type: ' + spec.ret);
    }};
  }}

  return wrapped;
}}

const env = {{ ...wrapUserEnv(userEnv), print }};

const {{ instance }} = await WebAssembly.instantiate(wasmBytes, {{ env }});
const memory = instance.exports.memory;
bytes = new Uint8Array(memory.buffer);
decoder = new TextDecoder('utf-8');
allocFn = instance.exports.__alloc;

const ret = instance.exports.main();
console.log(ret);
"#
    )
}

fn browser_app_mjs() -> &'static str {
    r#"const wasmBytes = await (await fetch('./out.wasm')).arrayBuffer();

let bytes;
let decoder;
function readString(ptr, len) {
  return decoder.decode(bytes.subarray(ptr, ptr + len));
}
function print(ptr, len) {
  console.log(readString(ptr, len));
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
