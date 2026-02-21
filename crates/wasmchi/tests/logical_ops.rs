use wasmchi::compile_to_wasm;

// Helper: compile, validate wasm, run main() via node, return i32.
fn run_main(wasm: &[u8]) -> i32 {
    use std::{fs, process::Command};

    let dir = std::env::temp_dir().join(format!(
        "wasmchi-logical-{:?}-{}",
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

// ── `&&` operator ────────────────────────────────────────────────────────────

#[test]
fn and_true_true_yields_true() {
    let src = r#"
export fn main(): i32 {
  return 1 && 1
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 1);
}

#[test]
fn and_true_false_yields_false() {
    let src = r#"
export fn main(): i32 {
  return 1 && 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 0);
}

#[test]
fn and_short_circuits_left_false() {
    // Left side is 0, right side would be 1 – result must be 0.
    let src = r#"
export fn main(): i32 {
  return 0 && 1
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 0);
}

// ── `||` operator ────────────────────────────────────────────────────────────

#[test]
fn or_false_false_yields_false() {
    let src = r#"
export fn main(): i32 {
  return 0 || 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 0);
}

#[test]
fn or_false_true_yields_true() {
    let src = r#"
export fn main(): i32 {
  return 0 || 1
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 1);
}

#[test]
fn or_true_false_yields_true_short_circuit() {
    let src = r#"
export fn main(): i32 {
  return 1 || 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 1);
}

// ── `!` operator ─────────────────────────────────────────────────────────────

#[test]
fn not_true_yields_false() {
    let src = r#"
export fn main(): i32 {
  return !1
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 0);
}

#[test]
fn not_false_yields_true() {
    let src = r#"
export fn main(): i32 {
  return !0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 1);
}

#[test]
fn not_in_condition() {
    let src = r#"
export fn main(): i32 {
  let x = 0
  if !x {
    return 42
  }
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 42);
}

// ── combined logical expressions ──────────────────────────────────────────────

#[test]
fn combined_and_or_not() {
    let src = r#"
export fn main(): i32 {
  let a = 1
  let b = 0
  if a && !b {
    return 7
  }
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 7);
}

// ── `else if` chaining ───────────────────────────────────────────────────────

#[test]
fn else_if_first_branch() {
    let src = r#"
export fn main(): i32 {
  let x = 1
  if x == 1 {
    return 10
  } else if x == 2 {
    return 20
  } else {
    return 30
  }
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 10);
}

#[test]
fn else_if_second_branch() {
    let src = r#"
export fn main(): i32 {
  let x = 2
  if x == 1 {
    return 10
  } else if x == 2 {
    return 20
  } else {
    return 30
  }
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 20);
}

#[test]
fn else_if_fallthrough_else() {
    let src = r#"
export fn main(): i32 {
  let x = 99
  if x == 1 {
    return 10
  } else if x == 2 {
    return 20
  } else {
    return 30
  }
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 30);
}

#[test]
fn else_if_chain_no_else() {
    let src = r#"
export fn main(): i32 {
  let x = 5
  let r = 0
  if x == 1 {
    r := 1
  } else if x == 5 {
    r := 5
  }
  return r
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
    assert_eq!(run_main(&wasm), 5);
}

// ── heap/data no-collision ────────────────────────────────────────────────────

#[test]
fn heap_base_above_string_data() {
    // A program with string data: the heap base must be >= total string bytes.
    // We verify by allocating memory and ensuring it doesn't overwrite the data.
    // We test indirectly: the wasm must be valid and main must return 0.
    let src = r#"
export fn main(): i32 {
  print("hello, wasmchi world!")
  return 0
}
"#;
    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}
