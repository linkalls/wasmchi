use wasmchi::{parse, Expr, Item, Program, Stmt, Type};

#[test]
fn parses_v_style_fn_signature_and_short_decl() {
    let src = "export fn main() i32 {\n  x := 1\n  print(\"h\"+\"i\")\n  return x + 2\n}\n";
    let got = parse(src).unwrap();

    let want = Program {
        items: vec![Item::ExportFn(wasmchi::Function {
            name: "main".to_string(),
            params: vec![],
            ret_ty: Type::I32,
            body: vec![
                Stmt::Let { name: "x".to_string(), expr: Expr::Int(1) },
                Stmt::Print(Expr::Str("hi".to_string())),
                Stmt::Return(Expr::Binary {
                    op: wasmchi::BinOp::Add,
                    left: Box::new(Expr::Var("x".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            ],
        })],
    };

    assert_eq!(got, want);
}
