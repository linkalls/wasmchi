use thiserror::Error;

use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ExportKind, ExportSection, Function, FunctionSection,
    ImportSection, Instruction, MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::ast::*;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("missing export fn main")]
    MissingMain,
    #[error("only i32 return type supported in v0")]
    UnsupportedType,
    #[error("function body must end in return")]
    MissingReturn,
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
    #[error("print only supports string literal in v0")]
    PrintNonLiteral,
}

pub fn emit_module(program: &Program) -> Result<Vec<u8>, CodegenError> {
    let Some(main) = program.items.iter().find_map(|it| match it {
        Item::ExportFn(f) if f.name == "main" => Some(f),
        _ => None,
    }) else {
        return Err(CodegenError::MissingMain);
    };

    if !main.params.is_empty() {
        // v0: main has no params
        return Err(CodegenError::UnsupportedType);
    }
    if main.ret_ty != Type::I32 {
        return Err(CodegenError::UnsupportedType);
    }

    let mut module = Module::new();

    // types: 0 = main() -> i32, 1 = print(i32,i32) -> void
    let mut types = TypeSection::new();
    let main_ty = types.len();
    types.ty().function([], [ValType::I32]);
    let print_ty = types.len();
    types.ty().function([ValType::I32, ValType::I32], []);
    module.section(&types);

    // imports
    let mut imports = ImportSection::new();
    imports.import("env", "print", wasm_encoder::EntityType::Function(print_ty));
    module.section(&imports);

    // functions
    let mut functions = FunctionSection::new();
    functions.function(main_ty);
    module.section(&functions);

    // memory (1 page) + export
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memories);

    // exports: main func index is 1 (after imported print=0); memory index 0
    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 1);
    exports.export("memory", ExportKind::Memory, 0);
    module.section(&exports);

    // data for string literals
    let mut data = DataSection::new();
    let mut next_data_offset: u32 = 0;

    // code
    let local_count = main
        .body
        .iter()
        .filter(|s| matches!(s, Stmt::Let { .. }))
        .count() as u32;

    let mut codes = CodeSection::new();
    let mut f = Function::new([(local_count, ValType::I32)]);

    let mut locals = std::collections::HashMap::<String, u32>::new();
    let mut next_local: u32 = 0;

    for stmt in &main.body {
        match stmt {
            Stmt::Let { name, expr } => {
                let local_idx = next_local;
                next_local += 1;
                locals.insert(name.clone(), local_idx);
                emit_expr(&mut f, expr, &locals)?;
                f.instruction(&Instruction::LocalSet(local_idx));
            }
            Stmt::Print(expr) => {
                let Expr::Str(s) = expr else {
                    return Err(CodegenError::PrintNonLiteral);
                };
                let bytes = s.as_bytes();
                let ptr = next_data_offset;
                let len = bytes.len() as u32;
                next_data_offset = next_data_offset.saturating_add(len);

                // init data segment at ptr
                let offset = ConstExpr::i32_const(ptr as i32);
                data.active(0, &offset, bytes.iter().copied());

                // call env.print(ptr, len)
                f.instruction(&Instruction::I32Const(ptr as i32));
                f.instruction(&Instruction::I32Const(len as i32));
                f.instruction(&Instruction::Call(0));
            }
            Stmt::Return(expr) => {
                emit_expr(&mut f, expr, &locals)?;
                f.instruction(&Instruction::End);
                codes.function(&f);
                module.section(&codes);
                module.section(&data);
                return Ok(module.finish());
            }
            Stmt::Expr(expr) => {
                emit_expr(&mut f, expr, &locals)?;
                f.instruction(&Instruction::Drop);
            }
        }
    }

    Err(CodegenError::MissingReturn)
}

fn emit_expr(
    f: &mut Function,
    expr: &Expr,
    locals: &std::collections::HashMap<String, u32>,
) -> Result<(), CodegenError> {
    match expr {
        Expr::Int(n) => {
            f.instruction(&Instruction::I32Const(*n));
            Ok(())
        }
        Expr::Str(_) => Err(CodegenError::UnsupportedType),
        Expr::Var(name) => {
            let Some(idx) = locals.get(name) else {
                return Err(CodegenError::UnknownVariable(name.clone()));
            };
            f.instruction(&Instruction::LocalGet(*idx));
            Ok(())
        }
        Expr::Binary { op, left, right } => {
            emit_expr(f, left, locals)?;
            emit_expr(f, right, locals)?;
            match op {
                BinOp::Add => f.instruction(&Instruction::I32Add),
                BinOp::Sub => f.instruction(&Instruction::I32Sub),
                BinOp::Mul => f.instruction(&Instruction::I32Mul),
                BinOp::Div => f.instruction(&Instruction::I32DivS),
            };
            Ok(())
        }
    }
}
