use wasmchi::compile_to_wasm;

#[test]
fn can_define_non_export_fn_and_call_from_main() {
    let src = r#"
fn add(a: i32, b: i32): i32 {
  return a + b
}

export fn main(): i32 {
  return add(40, 2)
}
"#;

    let wasm = compile_to_wasm(src).unwrap();
    wasmparser::validate(&wasm).unwrap();
}
