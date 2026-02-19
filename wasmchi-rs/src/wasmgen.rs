use anyhow::{bail, Context, Result};
use wasm_encoder::{CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection, Instruction, MemorySection, Module, TypeSection, ValType};

use crate::parse::{BinOp, Expr, FnDecl, Program, Stmt, TypeName};

#[derive(Debug, Clone, Copy)]
pub enum Target {
    Web,
    Wasi,
}

fn valtype(t: &TypeName) -> Option<ValType> {
    match t {
        TypeName::I32 => Some(ValType::I32),
        TypeName::Void => None,
    }
}

fn fold_const(expr: &Expr, consts: &std::collections::HashMap<String, i32>) -> Option<i32> {
    match expr {
        Expr::Num(n) => Some(*n),
        Expr::Var(s) => consts.get(s).copied(),
        Expr::Call { .. } => None,
        Expr::Bin { op, l, r } => {
            let a = fold_const(l, consts)?;
            let b = fold_const(r, consts)?;
            Some(match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::And => a & b,
                BinOp::Or => a | b,
                BinOp::Xor => a ^ b,
                BinOp::Shl => a.wrapping_shl((b & 31) as u32),
                BinOp::Shr => ((a as i32) >> (b & 31)) as i32,
                BinOp::Eq => if a == b { 1 } else { 0 },
                BinOp::Ne => if a != b { 1 } else { 0 },
                BinOp::Lt => if a < b { 1 } else { 0 },
                BinOp::Le => if a <= b { 1 } else { 0 },
                BinOp::Gt => if a > b { 1 } else { 0 },
                BinOp::Ge => if a >= b { 1 } else { 0 },
            })
        }
    }
}

fn ensure_start(prog: &mut Program) {
    if prog.fns.iter().any(|f| f.name == "_start") {
        return;
    }
    // export _start by default
    prog.fns.insert(
        0,
        FnDecl {
            exported: true,
            name: "_start".to_string(),
            params: vec![],
            ret: TypeName::Void,
            body: vec![
                // store_i32(0,1024);
                Stmt::Expr(Expr::Call {
                    name: "store_i32".to_string(),
                    args: vec![Expr::Num(0), Expr::Num(1024)],
                }),
                Stmt::Return(None),
            ],
        },
    );
}

pub fn compile(mut prog: Program, target: Target) -> Result<Vec<u8>> {
    ensure_start(&mut prog);

    // const fold map (order matters; resolve sequentially)
    let mut consts = std::collections::HashMap::<String, i32>::new();
    for c in &prog.consts {
        let v = fold_const(&c.expr, &consts)
            .with_context(|| format!("const {} not foldable yet", c.name))?;
        consts.insert(c.name.clone(), v);
    }

    // types
    let mut types = TypeSection::new();
    let mut type_index = std::collections::HashMap::<(Vec<TypeName>, TypeName), u32>::new();
    let mut get_type = |params: &[TypeName], ret: &TypeName| -> u32 {
        let key = (params.to_vec(), ret.clone());
        if let Some(i) = type_index.get(&key) {
            return *i;
        }
        let p: Vec<ValType> = params.iter().filter_map(valtype).collect();
        let r: Vec<ValType> = valtype(ret).into_iter().collect();
        let idx = types.len();
        {
            let mut enc = types.ty();
            enc.function(p, r);
        }
        type_index.insert(key, idx);
        idx
    };

    // function indices
    let mut func_index = std::collections::HashMap::<String, u32>::new();
    for f in &prog.fns {
        let idx = func_index.len() as u32;
        func_index.insert(f.name.clone(), idx);
    }

    let mut functions = FunctionSection::new();
    for f in &prog.fns {
        let ps: Vec<TypeName> = f.params.iter().map(|(_, t)| t.clone()).collect();
        let ti = get_type(&ps, &f.ret);
        functions.function(ti);
    }

    // memory export always
    let mut memories = MemorySection::new();
    memories.memory(wasm_encoder::MemoryType {
        minimum: 32,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);

    // exports from source
    for f in &prog.fns {
        if f.exported {
            let idx = *func_index.get(&f.name).unwrap();
            exports.export(&f.name, ExportKind::Func, idx);
        }
    }

    // ergonomics: if no source exports, export entry
    if exports.len() == 1 {
        match target {
            Target::Web => {
                let idx = *func_index.get("main").context("missing fn main")?;
                exports.export("main", ExportKind::Func, idx);
            }
            Target::Wasi => {
                let idx = *func_index.get("_start").context("missing fn _start")?;
                exports.export("_start", ExportKind::Func, idx);
            }
        }
    }

    // codegen
    let mut code = CodeSection::new();
    for f in &prog.fns {
        let mut locals_map = std::collections::HashMap::<String, u32>::new();
        for (i, (n, _)) in f.params.iter().enumerate() {
            locals_map.insert(n.clone(), i as u32);
        }
        let mut next_local = f.params.len() as u32;
        let mut locals_types: Vec<ValType> = Vec::new();

        let mut func = Function::new(vec![]);

        // helper: allocate a new i32 local
        let mut new_local = |locals_types: &mut Vec<ValType>, next_local: &mut u32, locals_map: &mut std::collections::HashMap<String, u32>, name: Option<String>| -> u32 {
            let idx = *next_local;
            *next_local += 1;
            locals_types.push(ValType::I32);
            if let Some(n) = name {
                locals_map.insert(n, idx);
            }
            idx
        };

        // We'll append instructions then finalize locals with a single i32 group.
        let mut instrs: Vec<Instruction> = Vec::new();

        fn emit_expr(
            e: &Expr,
            instrs: &mut Vec<Instruction>,
            locals: &std::collections::HashMap<String, u32>,
            func_index: &std::collections::HashMap<String, u32>,
            consts: &std::collections::HashMap<String, i32>,
        ) -> Result<()> {
            match e {
                Expr::Num(n) => {
                    instrs.push(Instruction::I32Const(*n));
                }
                Expr::Var(name) => {
                    if let Some(v) = consts.get(name) {
                        instrs.push(Instruction::I32Const(*v));
                    } else if let Some(i) = locals.get(name) {
                        instrs.push(Instruction::LocalGet(*i));
                    } else {
                        bail!("undefined var {name}");
                    }
                }
                Expr::Bin { op, l, r } => {
                    emit_expr(l, instrs, locals, func_index, consts)?;
                    emit_expr(r, instrs, locals, func_index, consts)?;
                    instrs.push(match op {
                        BinOp::Add => Instruction::I32Add,
                        BinOp::Sub => Instruction::I32Sub,
                        BinOp::Mul => Instruction::I32Mul,
                        BinOp::And => Instruction::I32And,
                        BinOp::Or => Instruction::I32Or,
                        BinOp::Xor => Instruction::I32Xor,
                        BinOp::Shl => Instruction::I32Shl,
                        BinOp::Shr => Instruction::I32ShrS,
                        BinOp::Eq => Instruction::I32Eq,
                        BinOp::Ne => Instruction::I32Ne,
                        BinOp::Lt => Instruction::I32LtS,
                        BinOp::Le => Instruction::I32LeS,
                        BinOp::Gt => Instruction::I32GtS,
                        BinOp::Ge => Instruction::I32GeS,
                    });
                }
                Expr::Call { name, args } => {
                    if name == "load_i32" {
                        if args.len() != 1 {
                            bail!("load_i32 expects 1 arg");
                        }
                        emit_expr(&args[0], instrs, locals, func_index, consts)?;
                        instrs.push(Instruction::I32Load(wasm_encoder::MemArg {
                            offset: 0,
                            align: 2,
                            memory_index: 0,
                        }));
                        return Ok(());
                    }
                    if name == "store_i32" {
                        if args.len() != 2 {
                            bail!("store_i32 expects 2 args");
                        }
                        emit_expr(&args[0], instrs, locals, func_index, consts)?;
                        emit_expr(&args[1], instrs, locals, func_index, consts)?;
                        instrs.push(Instruction::I32Store(wasm_encoder::MemArg {
                            offset: 0,
                            align: 2,
                            memory_index: 0,
                        }));
                        return Ok(());
                    }

                    let idx = *func_index.get(name).with_context(|| format!("unknown fn {name}"))?;
                    for a in args {
                        emit_expr(a, instrs, locals, func_index, consts)?;
                    }
                    instrs.push(Instruction::Call(idx));
                }
            }
            Ok(())
        }

        fn emit_stmt(
            st: &Stmt,
            instrs: &mut Vec<Instruction>,
            locals_map: &mut std::collections::HashMap<String, u32>,
            func_index: &std::collections::HashMap<String, u32>,
            consts: &std::collections::HashMap<String, i32>,
            locals_types: &mut Vec<ValType>,
            next_local: &mut u32,
        ) -> Result<()> {
            match st {
                Stmt::Let { name, ty: _, expr } => {
                    let idx = *next_local;
                    *next_local += 1;
                    locals_types.push(ValType::I32);
                    locals_map.insert(name.clone(), idx);
                    emit_expr(expr, instrs, locals_map, func_index, consts)?;
                    instrs.push(Instruction::LocalSet(idx));
                }
                Stmt::Assign { name, expr } => {
                    let idx = *locals_map.get(name).with_context(|| format!("undef var {name}"))?;
                    emit_expr(expr, instrs, locals_map, func_index, consts)?;
                    instrs.push(Instruction::LocalSet(idx));
                }
                Stmt::Expr(e) => {
                    emit_expr(e, instrs, locals_map, func_index, consts)?;
                    // if the expr returns a value we should drop; currently only calls/loads used.
                    // For safety, drop if it could leave value on stack: load_i32 and non-void calls.
                    // We'll only drop on load_i32.
                    if matches!(e, Expr::Call { name, .. } if name == "load_i32") {
                        instrs.push(Instruction::Drop);
                    }
                }
                Stmt::Return(opt) => {
                    if let Some(e) = opt {
                        emit_expr(e, instrs, locals_map, func_index, consts)?;
                    }
                    instrs.push(Instruction::Return);
                }
                Stmt::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    emit_expr(cond, instrs, locals_map, func_index, consts)?;
                    instrs.push(Instruction::If(wasm_encoder::BlockType::Empty));
                    for s in then_body {
                        emit_stmt(s, instrs, locals_map, func_index, consts, locals_types, next_local)?;
                    }
                    if !else_body.is_empty() {
                        instrs.push(Instruction::Else);
                        for s in else_body {
                            emit_stmt(s, instrs, locals_map, func_index, consts, locals_types, next_local)?;
                        }
                    }
                    instrs.push(Instruction::End);
                }
                Stmt::While { cond, body } => {
                    instrs.push(Instruction::Block(wasm_encoder::BlockType::Empty));
                    instrs.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
                    emit_expr(cond, instrs, locals_map, func_index, consts)?;
                    instrs.push(Instruction::I32Eqz);
                    instrs.push(Instruction::BrIf(1));
                    for s in body {
                        emit_stmt(s, instrs, locals_map, func_index, consts, locals_types, next_local)?;
                    }
                    instrs.push(Instruction::Br(0));
                    instrs.push(Instruction::End);
                    instrs.push(Instruction::End);
                }
            }
            Ok(())
        }

        // emit function body
        for st in &f.body {
            emit_stmt(
                st,
                &mut instrs,
                &mut locals_map,
                &func_index,
                &consts,
                &mut locals_types,
                &mut next_local,
            )?;
        }

        instrs.push(Instruction::End);

        // locals grouping: one group of i32 locals
        let local_count = locals_types.len() as u32;
        if local_count > 0 {
            func = Function::new(vec![(local_count, ValType::I32)]);
        }
        for ins in instrs {
            func.instruction(&ins);
        }

        code.function(&func);
    }

    let mut m = Module::new();
    m.section(&types);
    m.section(&functions);
    m.section(&memories);
    m.section(&exports);
    m.section(&code);
    Ok(m.finish())
}
