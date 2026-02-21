use thiserror::Error;

use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ExportKind, ExportSection, Function as WasmFunction,
    FunctionSection, ImportSection, Instruction, MemorySection, MemoryType, Module, TypeSection,
    ValType,
};

use crate::ast::{BinOp, Expr, Item, Program, Stmt, Type};
use crate::ast::Function as AstFunction;

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
    // Collect functions in source order; allow both exported and non-exported.
    let mut fns: Vec<(&AstFunction, bool)> = Vec::new();
    for item in &program.items {
        match item {
            Item::ExportFn(f) => fns.push((f, true)),
            Item::Fn(f) => fns.push((f, false)),
        }
    }

    let Some((main, _)) = fns.iter().find(|(f, _)| f.name == "main").copied() else {
        return Err(CodegenError::MissingMain);
    };

    if main.ret_ty != Type::I32 {
        return Err(CodegenError::UnsupportedType);
    }

    // v0: only i32 params/returns for user functions.
    for (f, _) in &fns {
        if f.ret_ty != Type::I32 {
            return Err(CodegenError::UnsupportedType);
        }
        for p in &f.params {
            if p.ty != Type::I32 {
                return Err(CodegenError::UnsupportedType);
            }
        }
    }

    let mut module = Module::new();

    // types
    let mut types = TypeSection::new();
    // type 0 reserved for env.print(ptr,len)
    let print_ty = types.len();
    types.ty().function([ValType::I32, ValType::I32], []);

    // function types for user fns (one per fn for now)
    let mut fn_type_indices: Vec<u32> = Vec::new();
    for (f, _) in &fns {
        let idx = types.len();
        let params = std::iter::repeat(ValType::I32).take(f.params.len());
        types.ty().function(params, [ValType::I32]);
        fn_type_indices.push(idx);
    }
    module.section(&types);

    // imports
    let mut imports = ImportSection::new();
    imports.import("env", "print", wasm_encoder::EntityType::Function(print_ty));
    module.section(&imports);

    // functions
    let mut functions = FunctionSection::new();
    for ty in &fn_type_indices {
        functions.function(*ty);
    }
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

    // exports
    let mut exports = ExportSection::new();
    // function index space: imported print = 0, then user fns start at 1 in the same order
    for (i, (f, is_export)) in fns.iter().enumerate() {
        if *is_export {
            exports.export(&f.name, ExportKind::Func, 1 + i as u32);
        }
    }
    exports.export("memory", ExportKind::Memory, 0);
    module.section(&exports);

    // data for string literals
    let mut data = DataSection::new();
    let mut next_data_offset: u32 = 0;

    // name -> func index
    let mut fn_indices = std::collections::HashMap::<String, u32>::new();
    for (i, (f, _)) in fns.iter().enumerate() {
        fn_indices.insert(f.name.clone(), 1 + i as u32);
    }

    // code
    let mut codes = CodeSection::new();

    for (_func_i, (fndef, _)) in fns.iter().enumerate() {
        let local_count = fndef
            .body
            .iter()
            .filter(|s| matches!(s, Stmt::Let { .. }))
            .count() as u32;

        let mut f = WasmFunction::new([(local_count, ValType::I32)]);

        let mut locals = std::collections::HashMap::<String, u32>::new();
        // params first
        for (pi, p) in fndef.params.iter().enumerate() {
            locals.insert(p.name.clone(), pi as u32);
        }
        let mut next_local: u32 = fndef.params.len() as u32;

        let mut did_return = false;

        for stmt in &fndef.body {
            match stmt {
                Stmt::Let { name, expr } => {
                    let local_idx = next_local;
                    next_local += 1;
                    locals.insert(name.clone(), local_idx);
                    emit_expr(&mut f, expr, &locals, &fn_indices)?;
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

                    let offset = ConstExpr::i32_const(ptr as i32);
                    data.active(0, &offset, bytes.iter().copied());

                    f.instruction(&Instruction::I32Const(ptr as i32));
                    f.instruction(&Instruction::I32Const(len as i32));
                    f.instruction(&Instruction::Call(0));
                }
                Stmt::Return(expr) => {
                    emit_expr(&mut f, expr, &locals, &fn_indices)?;
                    f.instruction(&Instruction::End);
                    did_return = true;
                    break;
                }
                Stmt::Expr(expr) => {
                    emit_expr(&mut f, expr, &locals, &fn_indices)?;
                    f.instruction(&Instruction::Drop);
                }
            }
        }

        if !did_return {
            return Err(CodegenError::MissingReturn);
        }

        codes.function(&f);
    }

    module.section(&codes);
    module.section(&data);
    Ok(module.finish())
}

fn emit_expr(
    f: &mut WasmFunction,
    expr: &Expr,
    locals: &std::collections::HashMap<String, u32>,
    fn_indices: &std::collections::HashMap<String, u32>,
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
        Expr::Call { callee, args } => {
            let Some(idx) = fn_indices.get(callee) else {
                return Err(CodegenError::UnknownVariable(callee.clone()));
            };
            for a in args {
                emit_expr(f, a, locals, fn_indices)?;
            }
            f.instruction(&Instruction::Call(*idx));
            Ok(())
        }
        Expr::Binary { op, left, right } => {
            emit_expr(f, left, locals, fn_indices)?;
            emit_expr(f, right, locals, fn_indices)?;
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
