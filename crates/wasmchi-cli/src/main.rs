mod auto_host;

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
                    wasmchi::Type::F64 => "f64",
                    wasmchi::Type::Bool => "i32",
                    wasmchi::Type::String => "string",
                    wasmchi::Type::JsObj => "jsobj",
                    wasmchi::Type::Void => "void",
                })
                .collect();
            let ret: &'static str = match im.ret_ty {
                wasmchi::Type::I32 => "i32",
                wasmchi::Type::F64 => "f64",
                wasmchi::Type::Bool => "i32",
                wasmchi::Type::String => "string",
                wasmchi::Type::JsObj => "jsobj",
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

fn auto_host_from_source(input_path: &PathBuf, src: &str) -> (String, Option<PathBuf>) {
    let imports = auto_host::parse_npm_imports(src);
    if imports.is_empty() {
        return (src.to_string(), None);
    }

    let directives = parse_wasmchi_directives(src);

    let project_dir = input_path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // deps: pin to "latest" for now
    let mut deps = std::collections::BTreeMap::<String, String>::new();
    for imp in &imports {
        deps.insert(imp.pkg.clone(), "latest".to_string());
    }

    auto_host::upsert_package_json_deps(project_dir, &deps);
    auto_host::ensure_bun_install(project_dir);

    // Infer signatures and rewrite source by replacing TS-like imports with `import fn`.
    let mut rewritten = String::new();
    let mut import_fns = Vec::new();

    // naive call-site arity scan (so optional params can be chosen when used)
    let mut call_arity = std::collections::HashMap::<String, usize>::new();
    for imp in &imports {
        for name in &imp.named {
            let maxa = max_call_arity(src, name);
            call_arity.insert(name.clone(), maxa);
        }
        if let Some(def) = &imp.default_name {
            let maxa = max_call_arity(src, def);
            call_arity.insert(def.clone(), maxa);
        }
    }

    for imp in &imports {
        if let Some(dts) = auto_host::read_pkg_dts(project_dir, &imp.pkg) {
            let mut all_names: Vec<String> = Vec::new();
            all_names.extend(imp.named.iter().cloned());
            if let Some(def) = &imp.default_name {
                all_names.push(def.clone());
            }

            for name in &all_names {
                if let Some(sig) = directives.sig_overrides.get(name) {
                    import_fns.push(format!("import fn {name}{}", sig));
                    continue;
                }

                if let Some((req, all, ret)) = auto_host::infer_fn_sig_with_optional(&dts, name) {
                    let want = *call_arity.get(name).unwrap_or(&req.len());
                    let mut chosen: Vec<&'static str> = if want <= req.len() {
                        req
                    } else {
                        all.into_iter().take(want).collect()
                    };
                    let mut ret = ret;

                    // Optional directive: treat TS `number` as i32 (i.e. replace f64 -> i32)
                    if directives.number_i32 {
                        chosen = chosen
                            .into_iter()
                            .map(|t| if t == "f64" { "i32" } else { t })
                            .collect();
                        if ret == "f64" {
                            ret = "i32";
                        }
                    }

                    let p = chosen
                        .into_iter()
                        .enumerate()
                        .map(|(i, ty)| format!("a{i}: {ty}"))
                        .collect::<Vec<_>>()
                        .join(", ");

                    import_fns.push(format!("import fn {name}({p}): {ret}"));
                } else {
                    // fallback
                    let mut ret = auto_host::infer_fn_ret_type(&dts, name).unwrap_or("i32");
                    if directives.number_i32 && ret == "f64" {
                        ret = "i32";
                    }
                    import_fns.push(format!("import fn {name}(): {ret}"));
                }
            }
        } else {
            // No d.ts: default to i32; better than nothing.
            for name in &imp.named {
                import_fns.push(format!("import fn {name}(): i32"));
            }
            if let Some(def) = &imp.default_name {
                import_fns.push(format!("import fn {def}(): i32"));
            }
        }
    }

    // Remove original TS-like import lines
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import") && trimmed.contains("from") && trimmed.contains("{") {
            continue;
        }
        if trimmed.starts_with("import") && trimmed.contains("{") && trimmed.contains("}") {
            // e.g. import { nanoid } from "npm:nanoid"
            continue;
        }
        if trimmed.starts_with("import") && trimmed.contains(" from ") {
            // e.g. import nanoid from "npm:nanoid"
            continue;
        }
        rewritten.push_str(line);
        rewritten.push('\n');
    }

    // Prepend import fn block
    let mut final_src = String::new();
    for l in import_fns {
        final_src.push_str(&l);
        final_src.push('\n');
    }
    final_src.push('\n');
    final_src.push_str(&rewritten);

    // Write auto host
    let tmp_dir = std::env::temp_dir().join("wasmchi");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let host = auto_host::write_auto_host(&tmp_dir, &imports);

    (final_src, Some(host))
}

#[derive(Default)]
struct WasmchiDirectives {
    number_i32: bool,
    // name -> "(a0: i32, ...): ret"
    sig_overrides: std::collections::HashMap<String, String>,
}

fn parse_wasmchi_directives(src: &str) -> WasmchiDirectives {
    let mut d = WasmchiDirectives::default();

    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("//") {
            continue;
        }
        let rest = line.trim_start_matches("//").trim();
        if !rest.starts_with("wasmchi:") {
            continue;
        }
        let rest = rest.trim_start_matches("wasmchi:").trim();

        if rest == "number=i32" {
            d.number_i32 = true;
            continue;
        }

        if let Some(spec) = rest.strip_prefix("sig ") {
            // format: sig name(<types...>): <ret>
            // example: sig nanoid(): string
            let spec = spec.trim();
            // find name
            let Some(paren) = spec.find('(') else { continue; };
            let name = spec[..paren].trim();
            let after = &spec[paren..];
            // accept exact suffix "(<...>): <ret>" or "(<...>) <ret>" by normalizing to ":"
            // We'll just store the tail starting from '('.
            if !name.is_empty() {
                // normalize `) <ret>` to `): <ret>` if needed
                let mut tail = after.to_string();
                if let Some(idx) = tail.rfind(") ") {
                    if !tail[idx..].contains(":") {
                        tail = format!("{}: {}", &tail[..idx + 1], tail[idx + 2..].trim());
                    }
                }
                d.sig_overrides.insert(name.to_string(), tail);
            }
        }
    }

    d
}

fn max_call_arity(src: &str, name: &str) -> usize {
    // super naive: count commas inside `name(...)` occurrences on a single line.
    // good enough for v0 samples.
    let mut maxa = 0usize;
    for line in src.lines() {
        let mut s = line;
        loop {
            let Some(i) = s.find(name) else { break; };
            s = &s[i + name.len()..];
            let s_trim = s.trim_start();
            if !s_trim.starts_with('(') {
                continue;
            }
            let mut depth = 0i32;
            let mut commas = 0usize;
            let mut started = false;
            for ch in s_trim.chars() {
                if ch == '(' {
                    depth += 1;
                    started = true;
                    continue;
                }
                if !started {
                    continue;
                }
                if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                if depth == 1 && ch == ',' {
                    commas += 1;
                }
            }
            // determine args: if we saw at least '()' then started true.
            // empty args => 0, else commas+1
            // heuristic: if there's any non-space between parens
            if let Some(end) = s_trim.find(')') {
                let inside = &s_trim[1..end].trim();
                let argc = if inside.is_empty() { 0 } else { commas + 1 };
                maxa = maxa.max(argc);
            }
        }
    }
    maxa
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

            let src = read_source(&input_path);
            let wasm = compile_source(&src);

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
        "doctor" => {
            run_doctor();
        }
        _ => usage_and_exit(),
    }
}

fn read_source(path: &PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("failed to read {}: {e}", path.display());
        process::exit(1);
    })
}

fn compile_source(src: &str) -> Vec<u8> {
    wasmchi::compile_to_wasm(src).unwrap_or_else(|e| {
        eprintln!("compile error: {e}");
        process::exit(1);
    })
}

fn run_node(input_path: PathBuf, host_module: Option<PathBuf>) {
    let src = read_source(&input_path);

    let mut host_module = host_module.and_then(|p| p.canonicalize().ok());

    // Auto-host for TS-like npm imports: `import { x } from "npm:pkg"`
    // If user didn't pass --host, we generate one.
    let (src2, auto_host) = auto_host_from_source(&input_path, &src);
    if host_module.is_none() {
        if let Some(p) = auto_host {
            host_module = Some(p);
        }
    }

    if std::env::var("WASMCHI_DUMP_SRC").is_ok() {
        eprintln!("--- wasmchi source (transformed) ---\n{src2}\n---");
    }

    // Compile transformed source.
    let wasm = compile_source(&src2);
    let src_final = src2;

    let import_meta = import_meta_from_source(&src_final);

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
    let src = read_source(&input_path);
    let wasm = compile_source(&src);

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
          args.push(loweredArgs[i] | 0);
          i += 1;
        }} else if (t === 'f64') {{
          args.push(+loweredArgs[i]);
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
      if (spec.ret === 'f64') {{
        return +ret;
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

const js_get = (_obj, _propPtr, _propLen) => 0;
const js_call0 = (_fn, _thisObj) => 0;

const env = {{ ...wrapUserEnv(userEnv), print, js_get, js_call0 }};

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

const js_get = (_obj, _propPtr, _propLen) => 0;
const js_call0 = (_fn, _thisObj) => 0;

const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { print, js_get, js_call0 } });
const memory = instance.exports.memory;
bytes = new Uint8Array(memory.buffer);
decoder = new TextDecoder('utf-8');

const ret = instance.exports.main();
console.log(ret);
"#
}

fn run_doctor() {
    fn check_tool(name: &str, version_arg: &str) {
        match std::process::Command::new(name).arg(version_arg).output() {
            Ok(out) if out.status.success() => {
                let ver = String::from_utf8_lossy(&out.stdout);
                let ver = ver.trim();
                println!("[✓] {name} ({ver})");
            }
            Ok(_) => println!("[✗] {name} (returned error)"),
            Err(_) => println!("[✗] {name} (not found)"),
        }
    }
    println!("wasmchi doctor — checking dependencies");
    check_tool("node", "--version");
    check_tool("bun", "--version");
}

fn usage_and_exit() {
    eprintln!(
        "wasmchi (wip)\n\nUSAGE:\n  wasmchi build <file.wm> [--out <out.wasm>]\n  wasmchi run [--target node|browser] <file.wm>\n  wasmchi bundle --target browser <file.wm> [--out-dir <dir>]\n  wasmchi doctor\n",
    );
    process::exit(2);
}
