use thiserror::Error;

use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ExportKind, ExportSection, Function as WasmFunction,
    FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};

use crate::ast::{BinOp, Expr, ImportFn, Item, Program, Stmt, Type};
use crate::ast::Function as AstFunction;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("missing export fn main")]
    MissingMain,
    #[error("unsupported type or feature in v0")]
    UnsupportedType,
    #[error("function body must end in return")]
    MissingReturn,
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
    #[error("print only supports string literal in v0")]
    PrintNonLiteral,
}

pub fn emit_module(program: &Program) -> Result<Vec<u8>, CodegenError> {
    // Collect imports + functions in source order.
    let mut imports: Vec<&ImportFn> = Vec::new();
    let mut fns: Vec<(&AstFunction, bool)> = Vec::new();
    for item in &program.items {
        match item {
            Item::ImportFn(i) => imports.push(i),
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

    // v0: user functions are i32-only for now. imports: allow i32 + string params and i32/void/string returns.
    for imp in &imports {
        if !matches!(imp.ret_ty, Type::I32 | Type::Void | Type::String) {
            return Err(CodegenError::UnsupportedType);
        }
        for p in &imp.params {
            if !matches!(p.ty, Type::I32 | Type::String) {
                return Err(CodegenError::UnsupportedType);
            }
        }
    }
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

    // type indices for imported fns
    let mut import_type_indices: Vec<u32> = Vec::new();
    for imp in &imports {
        let idx = types.len();
        // string params lower to (i32,i32)
        let mut lowered_params: Vec<ValType> = Vec::new();
        for p in &imp.params {
            match p.ty {
                Type::I32 => lowered_params.push(ValType::I32),
                Type::String => {
                    lowered_params.push(ValType::I32);
                    lowered_params.push(ValType::I32);
                }
                _ => return Err(CodegenError::UnsupportedType),
            }
        }
        let results: Vec<ValType> = match imp.ret_ty {
            Type::I32 => vec![ValType::I32],
            Type::Void => vec![],
            Type::String => vec![ValType::I32, ValType::I32],
        };
        types.ty().function(lowered_params, results);
        import_type_indices.push(idx);
    }

    // function types for user fns
    let mut fn_type_indices: Vec<u32> = Vec::new();
    for (f, _) in &fns {
        let idx = types.len();
        let params = std::iter::repeat(ValType::I32).take(f.params.len());
        types.ty().function(params, [ValType::I32]);
        fn_type_indices.push(idx);
    }

    // type for allocator: __alloc(len:i32)->i32
    let alloc_ty = types.len();
    types.ty().function([ValType::I32], [ValType::I32]);

    module.section(&types);

    // imports
    let mut import_section = ImportSection::new();
    import_section.import("env", "print", wasm_encoder::EntityType::Function(print_ty));
    for (imp, ty) in imports.iter().zip(import_type_indices.iter()) {
        import_section.import("env", &imp.name, wasm_encoder::EntityType::Function(*ty));
    }
    module.section(&import_section);

    // functions (defined)
    let mut functions = FunctionSection::new();
    for ty in &fn_type_indices {
        functions.function(*ty);
    }
    // allocator function
    functions.function(alloc_ty);

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

    // globals: heap pointer for __alloc
    // v0: start at a fixed offset to avoid overlapping data segments
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(4096),
    );
    module.section(&globals);

    // exports
    let mut exports = ExportSection::new();
    // function index space: imported print=0, then imported fns, then defined fns
    let defined_base = 1 + imports.len() as u32;
    for (i, (f, is_export)) in fns.iter().enumerate() {
        if *is_export {
            exports.export(&f.name, ExportKind::Func, defined_base + i as u32);
        }
    }
    // allocator export
    exports.export("__alloc", ExportKind::Func, defined_base + fns.len() as u32);

    exports.export("memory", ExportKind::Memory, 0);
    module.section(&exports);

    // data for string literals
    let mut data = DataSection::new();
    let mut next_data_offset: u32 = 0;

    // name -> func index
    let mut fn_indices = std::collections::HashMap::<String, u32>::new();
    // name -> signature (for imports, needed to lower args)
    let mut import_sigs = std::collections::HashMap::<String, Vec<Type>>::new();
    let mut import_rets = std::collections::HashMap::<String, Type>::new();

    // imported functions
    for (i, imp) in imports.iter().enumerate() {
        fn_indices.insert(imp.name.clone(), 1 + i as u32);
        import_sigs.insert(imp.name.clone(), imp.params.iter().map(|p| p.ty.clone()).collect());
        import_rets.insert(imp.name.clone(), imp.ret_ty.clone());
    }
    // defined functions
    let defined_base = 1 + imports.len() as u32;
    for (i, (f, _)) in fns.iter().enumerate() {
        fn_indices.insert(f.name.clone(), defined_base + i as u32);
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
                    emit_expr(&mut f, expr, &locals, &fn_indices, &import_sigs, &mut data, &mut next_data_offset)?;
                    f.instruction(&Instruction::LocalSet(local_idx));
                }
                Stmt::Print(expr) => {
                    match expr {
                        Expr::Str(s) => {
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
                        Expr::Call { callee, .. } => {
                            // allow printing the result of a string-returning import.
                            if !matches!(import_rets.get(callee), Some(Type::String)) {
                                return Err(CodegenError::PrintNonLiteral);
                            }
                            emit_expr(
                                &mut f,
                                expr,
                                &locals,
                                &fn_indices,
                                &import_sigs,
                                &mut data,
                                &mut next_data_offset,
                            )?;
                            // stack has (ptr,len)
                            f.instruction(&Instruction::Call(0));
                        }
                        _ => return Err(CodegenError::PrintNonLiteral),
                    }
                }
                Stmt::Return(expr) => {
                    emit_expr(&mut f, expr, &locals, &fn_indices, &import_sigs, &mut data, &mut next_data_offset)?;
                    f.instruction(&Instruction::End);
                    did_return = true;
                    break;
                }
                Stmt::Expr(expr) => {
                    // If this is a void-returning import call, don't drop.
                    if let Expr::Call { callee, .. } = expr {
                        if let Some(Type::Void) = import_rets.get(callee) {
                            emit_expr(
                                &mut f,
                                expr,
                                &locals,
                                &fn_indices,
                                &import_sigs,
                                &mut data,
                                &mut next_data_offset,
                            )?;
                            continue;
                        }
                    }

                    emit_expr(
                        &mut f,
                        expr,
                        &locals,
                        &fn_indices,
                        &import_sigs,
                        &mut data,
                        &mut next_data_offset,
                    )?;
                    f.instruction(&Instruction::Drop);
                }
            }
        }

        if !did_return {
            return Err(CodegenError::MissingReturn);
        }

        codes.function(&f);
    }

    // allocator code (last defined fn)
    // fn __alloc(len:i32)->i32 { let old=heap; heap+=len; return old }
    let mut alloc_fn = WasmFunction::new([]);
    alloc_fn.instruction(&Instruction::GlobalGet(0));
    alloc_fn.instruction(&Instruction::LocalGet(0));
    alloc_fn.instruction(&Instruction::I32Add);
    alloc_fn.instruction(&Instruction::GlobalSet(0));
    alloc_fn.instruction(&Instruction::GlobalGet(0));
    alloc_fn.instruction(&Instruction::LocalGet(0));
    alloc_fn.instruction(&Instruction::I32Sub);
    alloc_fn.instruction(&Instruction::End);
    codes.function(&alloc_fn);

    module.section(&codes);
    module.section(&data);
    Ok(module.finish())
}

fn emit_expr(
    f: &mut WasmFunction,
    expr: &Expr,
    locals: &std::collections::HashMap<String, u32>,
    fn_indices: &std::collections::HashMap<String, u32>,
    import_sigs: &std::collections::HashMap<String, Vec<Type>>,
    data: &mut DataSection,
    next_data_offset: &mut u32,
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

            // Lower args for imports: string => (ptr,len)
            if let Some(sig) = import_sigs.get(callee) {
                if sig.len() != args.len() {
                    return Err(CodegenError::UnsupportedType);
                }
                for (arg, ty) in args.iter().zip(sig.iter()) {
                    match ty {
                        Type::I32 => {
                            emit_expr(f, arg, locals, fn_indices, import_sigs, data, next_data_offset)?;
                        }
                        Type::String => {
                            let Expr::Str(s) = arg else {
                                return Err(CodegenError::UnsupportedType);
                            };
                            let bytes = s.as_bytes();
                            let ptr = *next_data_offset;
                            let len = bytes.len() as u32;
                            *next_data_offset = next_data_offset.saturating_add(len);
                            let offset = ConstExpr::i32_const(ptr as i32);
                            data.active(0, &offset, bytes.iter().copied());
                            f.instruction(&Instruction::I32Const(ptr as i32));
                            f.instruction(&Instruction::I32Const(len as i32));
                        }
                        _ => return Err(CodegenError::UnsupportedType),
                    }
                }
            } else {
                // Defined functions: i32-only for now
                for a in args {
                    emit_expr(f, a, locals, fn_indices, import_sigs, data, next_data_offset)?;
                }
            }

            f.instruction(&Instruction::Call(*idx));
            Ok(())
        }
        Expr::Binary { op, left, right } => {
            emit_expr(f, left, locals, fn_indices, import_sigs, data, next_data_offset)?;
            emit_expr(f, right, locals, fn_indices, import_sigs, data, next_data_offset)?;
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
