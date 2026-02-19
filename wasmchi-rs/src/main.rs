use anyhow::{bail, Context, Result};
use clap::Parser;

mod lex;
mod parse;
mod wasmgen;

#[derive(Parser, Debug)]
#[command(about = "wasmchi: tiny V-ish language -> pure WebAssembly")]
struct Args {
    #[arg(long)]
    input: String,

    #[arg(long)]
    out: String,

    /// web exports main if no exports exist; wasi exports _start if no exports exist
    #[arg(long, default_value = "web")]
    target: String,

    /// Export the auto-generated _start function (useful for embedders like vitrio).
    /// Disable for slightly smaller binaries when you don't need _start.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    export_start: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let src = std::fs::read_to_string(&args.input)
        .with_context(|| format!("read input failed: {}", args.input))?;

    let program = parse::parse(&src).context("parse failed")?;

    let target = match args.target.as_str() {
        "web" => wasmgen::Target::Web,
        "wasi" => wasmgen::Target::Wasi,
        other => bail!("unknown --target: {other}"),
    };

    let bytes = wasmgen::compile(program, target, args.export_start).context("compile failed")?;
    std::fs::write(&args.out, bytes).with_context(|| format!("write out failed: {}", args.out))?;

    Ok(())
}
