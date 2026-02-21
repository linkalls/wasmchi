use wasmchi::compile_to_wasm;

#[test]
fn emits_valid_wasm_for_return_const() {
    let src = "export fn main(): i32 {\n  return 0\n}\n";
    let wasm = compile_to_wasm(src).unwrap();

    wasmparser::validate(&wasm).unwrap();
}

#[test]
fn emits_valid_wasm_for_print_and_let_and_add() {
    let src = "export fn main(): i32 {\n  print(\"h\" + \"i\")\n  let x = 40\n  return x + 2\n}\n";
    let wasm = compile_to_wasm(src).unwrap();

    wasmparser::validate(&wasm).unwrap();
}
