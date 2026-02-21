use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, process::Command};

/// Parse lines like: `import { nanoid, foo } from "npm:nanoid"`
pub fn parse_npm_imports(src: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with("import") {
            continue;
        }
        // very small parser; v0 only.
        // import { a, b } from "npm:pkg"
        let Some(brace_l) = line.find('{') else { continue; };
        let Some(brace_r) = line.find('}') else { continue; };
        let Some(from_i) = line.find("from") else { continue; };
        if from_i < brace_r { continue; }
        let names_part = &line[brace_l + 1..brace_r];
        let mut names = Vec::new();
        for n in names_part.split(',') {
            let n = n.trim();
            if !n.is_empty() {
                names.push(n.to_string());
            }
        }
        if names.is_empty() { continue; }
        // find first quote after from
        let rest = &line[from_i + 4..];
        let rest = rest.trim();
        let quote = rest.chars().next();
        let Some(q) = quote.filter(|c| *c == '"' || *c == '\'') else { continue; };
        let rest2 = &rest[1..];
        let Some(endq) = rest2.find(q) else { continue; };
        let spec = &rest2[..endq];
        let spec = spec.strip_prefix("npm:").unwrap_or(spec);
        out.push((spec.to_string(), names));
    }
    out
}

/// Extract a return type for a function from a .d.ts file.
/// v0 supports only: string|number|void.
pub fn infer_fn_ret_type(dts: &str, name: &str) -> Option<&'static str> {
    // naive scan
    // match: export function name(...): <ret>;
    let needle = format!("export function {name}");
    let pos = dts.find(&needle)?;
    let after = &dts[pos + needle.len()..];

    // find `):` which ends the param list
    let close_paren = after.find(")")?;
    let after_paren = after[close_paren + 1..].trim_start();
    let after_colon = after_paren.strip_prefix(":")?.trim_start();

    // take until `;` or newline
    let mut tok = String::new();
    for ch in after_colon.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            tok.push(ch);
        } else {
            break;
        }
    }

    match tok.as_str() {
        "string" => Some("string"),
        "number" => Some("i32"),
        "void" => Some("void"),
        other => {
            // Heuristic: `function nanoid<Type extends string>(...): Type`
            // Treat generic string-like return as string.
            let decl_head = &after[..close_paren];
            if decl_head.contains("extends string") {
                // return is a generic param name
                if other.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                    return Some("string");
                }
            }
            None
        }
    }
}

pub fn ensure_bun_install(project_dir: &Path) {
    let status = Command::new("bun")
        .current_dir(project_dir)
        .arg("install")
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("bun install failed: {s}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("failed to run bun: {e}");
            std::process::exit(1);
        }
    }
}

pub fn upsert_package_json_deps(project_dir: &Path, deps: &BTreeMap<String, String>) {
    let pkg_path = project_dir.join("package.json");
    let json = if pkg_path.exists() {
        fs::read_to_string(&pkg_path).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    // ultra-minimal JSON patching: if no package.json, create a clean one.
    // v0: we overwrite with a minimal file to guarantee correctness.
    let mut dep_lines = String::new();
    for (k, v) in deps {
        dep_lines.push_str(&format!("    \"{k}\": \"{v}\",\n"));
    }
    if dep_lines.ends_with(",\n") {
        dep_lines.truncate(dep_lines.len() - 2);
        dep_lines.push('\n');
    }

    let content = format!(
        "{{\n  \"name\": \"wasmchi-project\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"dependencies\": {{\n{dep_lines}  }}\n}}\n"
    );

    // only write if different-ish
    if json.trim() != content.trim() {
        fs::write(&pkg_path, content).unwrap();
    }
}

pub fn read_pkg_dts(project_dir: &Path, pkg: &str) -> Option<String> {
    // heuristic: node_modules/<pkg>/index.d.ts or <types> field is too much; v0 heuristic.
    let p = project_dir.join("node_modules").join(pkg).join("index.d.ts");
    fs::read_to_string(p).ok()
}

pub fn write_auto_host(tmp_dir: &Path, imports: &[(String, Vec<String>)]) -> PathBuf {
    let mut lines = String::new();
    for (pkg, names) in imports {
        lines.push_str("import {");
        for (i, n) in names.iter().enumerate() {
            if i > 0 { lines.push_str(", "); }
            lines.push_str(n);
        }
        lines.push_str(&format!("}} from '{pkg}'\n"));
    }

    lines.push_str("\nexport default {\n");
    for (_pkg, names) in imports {
        for n in names {
            lines.push_str(&format!("  {n}: {n},\n"));
        }
    }
    lines.push_str("}\n");

    let host_path = tmp_dir.join("auto-host.mjs");
    fs::write(&host_path, lines).unwrap();
    host_path
}
