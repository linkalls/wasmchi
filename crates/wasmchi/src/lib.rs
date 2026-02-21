pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;

pub use ast::*;

pub fn parse(source: &str) -> Result<ast::Program, parser::ParseError> {
    parser::parse_program(source)
}

pub fn compile_to_wasm(source: &str) -> Result<Vec<u8>, CompileError> {
    let program = parse(source)?;
    let wasm = codegen::emit_module(&program)?;
    Ok(wasm)
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Parse(#[from] parser::ParseError),
    #[error(transparent)]
    Codegen(#[from] codegen::CodegenError),
}
