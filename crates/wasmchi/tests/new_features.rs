use wasmchi::compile_to_wasm;

// ── if/else ─────────────────────────────────────────────────────────────────

#[test]
fn if_else_selects_correct_branch() {
    let src = r#"
export fn main(): i32 {
  let x = 0
  if 1 == 1 {
    x := 10
  } else {
    x := 20
  }
  return x
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    let result = run_wasm_main(&wasm);
    assert_eq!(result, 10);
}

#[test]
fn if_without_else_executes_body() {
    let src = r#"
export fn main(): i32 {
  let x = 5
  if x == 5 {
    x := 42
  }
  return x
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    let result = run_wasm_main(&wasm);
    assert_eq!(result, 42);
}

// ── while ────────────────────────────────────────────────────────────────────

#[test]
fn while_loop_counts_to_ten() {
    let src = r#"
export fn main(): i32 {
  let i = 0
  let sum = 0
  while i < 10 {
    i := i + 1
    sum := sum + i
  }
  return sum
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    let result = run_wasm_main(&wasm);
    // sum of 1..=10 = 55
    assert_eq!(result, 55);
}

// ── comparison operators ─────────────────────────────────────────────────────

#[test]
fn comparison_operators_produce_correct_i32() {
    let src = r#"
export fn eq_test(): i32 { return 3 == 3 }
export fn ne_test(): i32 { return 3 != 4 }
export fn lt_test(): i32 { return 2 < 5 }
export fn le_test(): i32 { return 5 <= 5 }
export fn gt_test(): i32 { return 7 > 3 }
export fn ge_test(): i32 { return 4 >= 4 }
export fn main(): i32 { return 0 }
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}

// ── bool type ────────────────────────────────────────────────────────────────

#[test]
fn bool_literals_parse_and_compile() {
    let src = r#"
export fn main(): i32 {
  let t = true
  let f = false
  if t {
    return 1
  }
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    let result = run_wasm_main(&wasm);
    assert_eq!(result, 1);
}

// ── f64 literals and operations ──────────────────────────────────────────────

#[test]
fn f64_return_type_compiles() {
    let src = r#"
export fn pi(): f64 {
  return 3.14
}
export fn main(): i32 {
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}

#[test]
fn f64_arithmetic_compiles() {
    let src = r#"
export fn add_f64(a: f64, b: f64): f64 {
  return a + b
}
export fn main(): i32 {
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}

// ── void return type ─────────────────────────────────────────────────────────

#[test]
fn void_user_fn_compiles() {
    let src = r#"
fn noop(): void {
}
export fn main(): i32 {
  noop()
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}

// ── helper ───────────────────────────────────────────────────────────────────

/// Execute `main()` in a minimal wasmtime-free environment via Node.js.
fn run_wasm_main(wasm: &[u8]) -> i32 {
    use std::{fs, process::Command};

    // Use a unique temp directory per call to avoid parallel-test collisions.
    let dir = std::env::temp_dir().join(format!(
        "wasmchi-new-features-{:?}-{}",
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let wasm_path = dir.join("out.wasm");
    fs::write(&wasm_path, wasm).unwrap();

    let host_path = dir.join("host.mjs");
    fs::write(
        &host_path,
        r#"import fs from 'node:fs/promises';
const wasm = await fs.readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(wasm, {
  env: { print: () => {} }
});
const ret = instance.exports.main();
process.stdout.write(String(ret) + "\n");
"#,
    )
    .unwrap();

    let out = Command::new("node")
        .arg(&host_path)
        .arg(&wasm_path)
        .output()
        .expect("node should run");

    assert!(out.status.success(), "node failed: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse::<i32>().expect("expected i32 from main()")
}
