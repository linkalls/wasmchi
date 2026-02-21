use thiserror::Error;

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, ExportKind, ExportSection,
    Function as WasmFunction, FunctionSection, GlobalSection, GlobalType, ImportSection,
    Instruction, MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::ast::{BinOp, Expr, ImportFn, Item, Program, Stmt, Type};
use crate::ast::Function as AstFunction;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("missing export fn main")]
    MissingMain,
    #[error("unsupported type or feature")]
    UnsupportedType,
    #[error("function body must end in return")]
    MissingReturn,
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
    #[error("print only supports string literal")]
    PrintNonLiteral,
}

// Infer the Wasm value type of an expression given the current type environment.
fn infer_val_type(
    expr: &Expr,
    locals: &std::collections::HashMap<String, (u32, ValType)>,
    import_rets: &std::collections::HashMap<String, Type>,
    fn_rets: &std::collections::HashMap<String, Type>,
) -> ValType {
    match expr {
        Expr::Int(_) | Expr::Bool(_) => ValType::I32,
        Expr::Float(_) => ValType::F64,
        Expr::Str(_) => ValType::I32,
        Expr::Var(name) => locals.get(name).map(|(_, vt)| *vt).unwrap_or(ValType::I32),
        Expr::Call { callee, .. } => {
            let ty = import_rets.get(callee).or_else(|| fn_rets.get(callee));
            match ty {
                Some(Type::F64) => ValType::F64,
                _ => ValType::I32,
            }
        }
        Expr::Binary { op, left, right } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                ValType::I32
            }
            _ => {
                let l = infer_val_type(left, locals, import_rets, fn_rets);
                let r = infer_val_type(right, locals, import_rets, fn_rets);
                if l == ValType::F64 || r == ValType::F64 {
                    ValType::F64
                } else {
                    ValType::I32
                }
            }
        },
    }
}

// Recursively collect unique let bindings (name, inferred ValType) in order of first occurrence.
fn collect_unique_lets(
    stmts: &[Stmt],
    locals: &mut std::collections::HashMap<String, (u32, ValType)>,
    import_rets: &std::collections::HashMap<String, Type>,
    fn_rets: &std::collections::HashMap<String, Type>,
    result: &mut Vec<(String, ValType)>,
    seen: &mut std::collections::HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, expr } => {
                if !seen.contains(name) {
                    let vt = infer_val_type(expr, locals, import_rets, fn_rets);
                    seen.insert(name.clone());
                    result.push((name.clone(), vt));
                    // placeholder index 0 for now; will be fixed after full collection
                    locals.insert(name.clone(), (0, vt));
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                collect_unique_lets(then_body, locals, import_rets, fn_rets, result, seen);
                if let Some(eb) = else_body {
                    collect_unique_lets(eb, locals, import_rets, fn_rets, result, seen);
                }
            }
            Stmt::While { body, .. } => {
                collect_unique_lets(body, locals, import_rets, fn_rets, result, seen);
            }
            _ => {}
        }
    }
}

// Check whether any path in stmts contains a return statement.
fn has_any_return(stmts: &[Stmt]) -> bool {
    for s in stmts {
        match s {
            Stmt::Return(_) => return true,
            Stmt::If { then_body, else_body, .. } => {
                if has_any_return(then_body)
                    || else_body.as_deref().map_or(false, has_any_return)
                {
                    return true;
                }
            }
            Stmt::While { body, .. } => {
                if has_any_return(body) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
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

    let Some(_) = fns.iter().find(|(f, _)| f.name == "main").copied() else {
        return Err(CodegenError::MissingMain);
    };

    // Validate imported fn types.
    for imp in &imports {
        if !matches!(imp.ret_ty, Type::I32 | Type::F64 | Type::Void | Type::String) {
            return Err(CodegenError::UnsupportedType);
        }
        for p in &imp.params {
            if !matches!(p.ty, Type::I32 | Type::F64 | Type::String) {
                return Err(CodegenError::UnsupportedType);
            }
        }
    }
    // Validate user-defined fn types: allow i32, f64, void, bool.
    for (f, _) in &fns {
        if !matches!(f.ret_ty, Type::I32 | Type::F64 | Type::Void | Type::Bool) {
            return Err(CodegenError::UnsupportedType);
        }
        for p in &f.params {
            if !matches!(p.ty, Type::I32 | Type::F64 | Type::Bool) {
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
        let mut lowered_params: Vec<ValType> = Vec::new();
        for p in &imp.params {
            match p.ty {
                Type::I32 => lowered_params.push(ValType::I32),
                Type::F64 => lowered_params.push(ValType::F64),
                Type::String => {
                    lowered_params.push(ValType::I32);
                    lowered_params.push(ValType::I32);
                }
                _ => return Err(CodegenError::UnsupportedType),
            }
        }
        let results: Vec<ValType> = match imp.ret_ty {
            Type::I32 => vec![ValType::I32],
            Type::F64 => vec![ValType::F64],
            Type::Void => vec![],
            Type::String => vec![ValType::I32, ValType::I32],
            _ => return Err(CodegenError::UnsupportedType),
        };
        types.ty().function(lowered_params, results);
        import_type_indices.push(idx);
    }

    // function types for user fns (now supports f64/void/bool)
    let mut fn_type_indices: Vec<u32> = Vec::new();
    for (f, _) in &fns {
        let idx = types.len();
        let params: Vec<ValType> = f
            .params
            .iter()
            .map(|p| match p.ty {
                Type::I32 | Type::Bool => ValType::I32,
                Type::F64 => ValType::F64,
                _ => unreachable!(),
            })
            .collect();
        let results: Vec<ValType> = match f.ret_ty {
            Type::I32 | Type::Bool => vec![ValType::I32],
            Type::F64 => vec![ValType::F64],
            Type::Void => vec![],
            _ => unreachable!(),
        };
        types.ty().function(params, results);
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
    let mut fn_rets = std::collections::HashMap::<String, Type>::new();

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
        fn_rets.insert(f.name.clone(), f.ret_ty.clone());
    }

    // code
    let mut codes = CodeSection::new();

    for (fndef, _) in &fns {
        let wasm_f = build_wasm_function(
            fndef,
            &fn_indices,
            &import_sigs,
            &import_rets,
            &fn_rets,
            &mut data,
            &mut next_data_offset,
        )?;
        codes.function(&wasm_f);
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

fn build_wasm_function(
    fndef: &AstFunction,
    fn_indices: &std::collections::HashMap<String, u32>,
    import_sigs: &std::collections::HashMap<String, Vec<Type>>,
    import_rets: &std::collections::HashMap<String, Type>,
    fn_rets: &std::collections::HashMap<String, Type>,
    data: &mut DataSection,
    next_data_offset: &mut u32,
) -> Result<WasmFunction, CodegenError> {
    // Build initial locals map from parameters.
    let mut locals = std::collections::HashMap::<String, (u32, ValType)>::new();
    for (i, p) in fndef.params.iter().enumerate() {
        let vt = match p.ty {
            Type::I32 | Type::Bool => ValType::I32,
            Type::F64 => ValType::F64,
            _ => unreachable!(),
        };
        locals.insert(p.name.clone(), (i as u32, vt));
    }

    // Collect all unique let bindings with inferred types.
    let mut let_bindings: Vec<(String, ValType)> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    collect_unique_lets(
        &fndef.body,
        &mut locals,
        import_rets,
        fn_rets,
        &mut let_bindings,
        &mut seen,
    );

    // Assign local indices: i32 locals first, then f64 locals (after params).
    let n_params = fndef.params.len() as u32;
    let i32_lets: Vec<&str> = let_bindings
        .iter()
        .filter(|(_, vt)| *vt == ValType::I32)
        .map(|(n, _)| n.as_str())
        .collect();
    let f64_lets: Vec<&str> = let_bindings
        .iter()
        .filter(|(_, vt)| *vt == ValType::F64)
        .map(|(n, _)| n.as_str())
        .collect();

    let i32_base = n_params;
    let f64_base = n_params + i32_lets.len() as u32;

    for (i, name) in i32_lets.iter().enumerate() {
        locals.insert(name.to_string(), (i32_base + i as u32, ValType::I32));
    }
    for (i, name) in f64_lets.iter().enumerate() {
        locals.insert(name.to_string(), (f64_base + i as u32, ValType::F64));
    }

    // Build WasmFunction with declared locals.
    let mut wasm_local_decls: Vec<(u32, ValType)> = Vec::new();
    if !i32_lets.is_empty() {
        wasm_local_decls.push((i32_lets.len() as u32, ValType::I32));
    }
    if !f64_lets.is_empty() {
        wasm_local_decls.push((f64_lets.len() as u32, ValType::F64));
    }
    let mut f = WasmFunction::new(wasm_local_decls);

    // Check that non-void functions have at least one return path.
    if fndef.ret_ty != Type::Void && !has_any_return(&fndef.body) {
        return Err(CodegenError::MissingReturn);
    }

    emit_stmts(
        &mut f,
        &fndef.body,
        &locals,
        fn_indices,
        import_sigs,
        import_rets,
        fn_rets,
        data,
        next_data_offset,
    )?;

    f.instruction(&Instruction::End);
    Ok(f)
}

#[allow(clippy::too_many_arguments)]
fn emit_stmts(
    f: &mut WasmFunction,
    stmts: &[Stmt],
    locals: &std::collections::HashMap<String, (u32, ValType)>,
    fn_indices: &std::collections::HashMap<String, u32>,
    import_sigs: &std::collections::HashMap<String, Vec<Type>>,
    import_rets: &std::collections::HashMap<String, Type>,
    fn_rets: &std::collections::HashMap<String, Type>,
    data: &mut DataSection,
    next_data_offset: &mut u32,
) -> Result<(), CodegenError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, expr } => {
                let (local_idx, _) = *locals
                    .get(name)
                    .ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?;
                emit_expr(f, expr, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                f.instruction(&Instruction::LocalSet(local_idx));
            }
            Stmt::Print(expr) => {
                match expr {
                    Expr::Str(s) => {
                        let bytes = s.as_bytes();
                        let ptr = *next_data_offset;
                        let len = bytes.len() as u32;
                        *next_data_offset = next_data_offset.saturating_add(len);
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
                        emit_expr(f, expr, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                        // stack has (ptr,len)
                        f.instruction(&Instruction::Call(0));
                    }
                    _ => return Err(CodegenError::PrintNonLiteral),
                }
            }
            Stmt::Return(expr) => {
                emit_expr(f, expr, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                f.instruction(&Instruction::Return);
            }
            Stmt::Expr(expr) => {
                // Determine how many values the expression leaves on the stack.
                let n_results = if let Expr::Call { callee, .. } = expr {
                    let ret_ty =
                        import_rets.get(callee).or_else(|| fn_rets.get(callee));
                    match ret_ty {
                        Some(Type::Void) => 0usize,
                        Some(Type::String) => 2,
                        _ => 1,
                    }
                } else {
                    1
                };
                emit_expr(f, expr, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                for _ in 0..n_results {
                    f.instruction(&Instruction::Drop);
                }
            }
            Stmt::If { cond, then_body, else_body } => {
                emit_expr(f, cond, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                f.instruction(&Instruction::If(BlockType::Empty));
                emit_stmts(f, then_body, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                if let Some(eb) = else_body {
                    f.instruction(&Instruction::Else);
                    emit_stmts(f, eb, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                }
                f.instruction(&Instruction::End);
            }
            Stmt::While { cond, body } => {
                // block $exit
                //   loop $repeat
                //     <cond>
                //     i32.eqz
                //     br_if $exit  (label 1 = outer block)
                //     <body>
                //     br $repeat   (label 0 = loop)
                //   end
                // end
                f.instruction(&Instruction::Block(BlockType::Empty));
                f.instruction(&Instruction::Loop(BlockType::Empty));
                emit_expr(f, cond, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                f.instruction(&Instruction::I32Eqz);
                f.instruction(&Instruction::BrIf(1));
                emit_stmts(f, body, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                f.instruction(&Instruction::Br(0));
                f.instruction(&Instruction::End); // end loop
                f.instruction(&Instruction::End); // end block
            }
        }
    }
    Ok(())
}

fn emit_expr(
    f: &mut WasmFunction,
    expr: &Expr,
    locals: &std::collections::HashMap<String, (u32, ValType)>,
    fn_indices: &std::collections::HashMap<String, u32>,
    import_sigs: &std::collections::HashMap<String, Vec<Type>>,
    import_rets: &std::collections::HashMap<String, Type>,
    fn_rets: &std::collections::HashMap<String, Type>,
    data: &mut DataSection,
    next_data_offset: &mut u32,
) -> Result<(), CodegenError> {
    match expr {
        Expr::Int(n) => {
            f.instruction(&Instruction::I32Const(*n));
            Ok(())
        }
        Expr::Float(bits) => {
            f.instruction(&Instruction::F64Const(f64::from_bits(*bits).into()));
            Ok(())
        }
        Expr::Bool(b) => {
            f.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }));
            Ok(())
        }
        Expr::Str(_) => Err(CodegenError::UnsupportedType),
        Expr::Var(name) => {
            let Some((idx, _)) = locals.get(name) else {
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
                            emit_expr(f, arg, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                        }
                        Type::F64 => {
                            let arg_vt = infer_val_type(arg, locals, import_rets, fn_rets);
                            emit_expr(f, arg, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                            if arg_vt == ValType::I32 {
                                f.instruction(&Instruction::F64ConvertI32S);
                            }
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
                // Defined functions: pass args with type coercion.
                for a in args {
                    emit_expr(f, a, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
                }
            }

            f.instruction(&Instruction::Call(*idx));
            Ok(())
        }
        Expr::Binary { op, left, right } => {
            let lt = infer_val_type(left, locals, import_rets, fn_rets);
            let rt = infer_val_type(right, locals, import_rets, fn_rets);
            let use_f64 = lt == ValType::F64 || rt == ValType::F64;

            emit_expr(f, left, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
            if use_f64 && lt == ValType::I32 {
                f.instruction(&Instruction::F64ConvertI32S);
            }

            emit_expr(f, right, locals, fn_indices, import_sigs, import_rets, fn_rets, data, next_data_offset)?;
            if use_f64 && rt == ValType::I32 {
                f.instruction(&Instruction::F64ConvertI32S);
            }

            match op {
                BinOp::Add => {
                    if use_f64 { f.instruction(&Instruction::F64Add); } else { f.instruction(&Instruction::I32Add); }
                }
                BinOp::Sub => {
                    if use_f64 { f.instruction(&Instruction::F64Sub); } else { f.instruction(&Instruction::I32Sub); }
                }
                BinOp::Mul => {
                    if use_f64 { f.instruction(&Instruction::F64Mul); } else { f.instruction(&Instruction::I32Mul); }
                }
                BinOp::Div => {
                    if use_f64 { f.instruction(&Instruction::F64Div); } else { f.instruction(&Instruction::I32DivS); }
                }
                BinOp::Eq => {
                    if use_f64 { f.instruction(&Instruction::F64Eq); } else { f.instruction(&Instruction::I32Eq); }
                }
                BinOp::Ne => {
                    if use_f64 { f.instruction(&Instruction::F64Ne); } else { f.instruction(&Instruction::I32Ne); }
                }
                BinOp::Lt => {
                    if use_f64 { f.instruction(&Instruction::F64Lt); } else { f.instruction(&Instruction::I32LtS); }
                }
                BinOp::Le => {
                    if use_f64 { f.instruction(&Instruction::F64Le); } else { f.instruction(&Instruction::I32LeS); }
                }
                BinOp::Gt => {
                    if use_f64 { f.instruction(&Instruction::F64Gt); } else { f.instruction(&Instruction::I32GtS); }
                }
                BinOp::Ge => {
                    if use_f64 { f.instruction(&Instruction::F64Ge); } else { f.instruction(&Instruction::I32GeS); }
                }
            }
            Ok(())
        }
    }
}

