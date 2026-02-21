use wasmchi::{parse, BinOp, Expr, Item, Program, Stmt, Type};

#[test]
fn parses_export_fn_main_empty_body() {
    let src = "export fn main(): i32 {}\n";
    let got = parse(src).unwrap();

    let want = Program {
        items: vec![Item::ExportFn(wasmchi::Function {
            name: "main".to_string(),
            params: vec![],
            ret_ty: Type::I32,
            body: vec![],
        })],
    };

    assert_eq!(got, want);
}

#[test]
fn newline_separates_statements() {
    let src = "export fn main(): i32 {\n  let x = 1\n  print(\"o\" + \"k\")\n  return x + 2\n}\n";
    let got = parse(src).unwrap();

    let want = Program {
        items: vec![Item::ExportFn(wasmchi::Function {
            name: "main".to_string(),
            params: vec![],
            ret_ty: Type::I32,
            body: vec![
                Stmt::Let { name: "x".to_string(), expr: Expr::Int(1) },
                Stmt::Print(Expr::Str("ok".to_string())),
                Stmt::Return(Expr::Binary {
                    op: BinOp::Add,
                    left: Box::new(Expr::Var("x".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            ],
        })],
    };

    assert_eq!(got, want);
}
