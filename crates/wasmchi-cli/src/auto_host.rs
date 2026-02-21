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

/// Infer an exported function signature from a .d.ts file.
///
/// v0 supports only a small type surface:
/// - params: string | number | void (void never used as param)
/// - returns: string | number | void
/// where number => i32.
///
/// Supported patterns:
/// - `export function name(...): Ret`
/// - `export const name: (... ) => Ret`
///
/// Overloads:
/// - we pick the *first* overload that we can fully lower.
pub fn infer_fn_sig(dts: &str, name: &str) -> Option<(Vec<&'static str>, &'static str)> {
    infer_fn_sig_with_optional(dts, name).map(|(req, _all, ret)| (req, ret))
}

/// Like `infer_fn_sig`, but also returns the full param list including optional params.
/// Return: (required_params, all_params, ret)
pub fn infer_fn_sig_with_optional(
    dts: &str,
    name: &str,
) -> Option<(Vec<&'static str>, Vec<&'static str>, &'static str)> {
    // 1) Try `export function` (support overloads by scanning all occurrences)
    let needle = format!("export function {name}");
    let mut search_from = 0;
    while let Some(pos) = dts[search_from..].find(&needle) {
        let pos = search_from + pos;
        if let Some(sig) = infer_sig_from_export_function_at(dts, pos + needle.len()) {
            return Some(sig);
        }
        search_from = pos + needle.len();
    }

    // 2) Try `export const name: (... ) => Ret`
    let needle = format!("export const {name}");
    if let Some(pos) = dts.find(&needle) {
        let after = &dts[pos + needle.len()..];
        // find `=>`
        let arrow = after.find("=>")?;
        let after_arrow = after[arrow + 2..].trim_start();
        let ret_tok = take_ident(after_arrow);
        let ret = map_ret_token(ret_tok, after, None)?;

        // params: find first '(' after ':' and the matching ')'
        let open = after.find('(')?;
        let close = after[open + 1..].find(')')? + open + 1;
        let params_slice = &after[open + 1..close];
        let (req, all) = infer_params_with_optional(params_slice)?;

        return Some((req, all, ret));
    }

    None
}

/// Back-compat helper for older call sites.
pub fn infer_fn_ret_type(dts: &str, name: &str) -> Option<&'static str> {
    infer_fn_sig(dts, name).map(|(_, r)| r)
}

fn infer_sig_from_export_function_at(
    dts: &str,
    start: usize,
) -> Option<(Vec<&'static str>, Vec<&'static str>, &'static str)> {
    let after = &dts[start..];

    // params are inside (...)
    let open_paren = after.find('(')?;
    let close_paren = after.find(")")?;
    if close_paren < open_paren {
        return None;
    }
    let params_slice = &after[open_paren + 1..close_paren];
    let (req, all) = infer_params_with_optional(params_slice)?;

    // return after `):`
    let after_paren = after[close_paren + 1..].trim_start();
    let after_colon = after_paren.strip_prefix(":")?.trim_start();
    let ret_tok = take_ident(after_colon);
    let ret = map_ret_token(ret_tok, after, Some(close_paren))?;

    Some((req, all, ret))
}

fn infer_params_with_optional(params_slice: &str) -> Option<(Vec<&'static str>, Vec<&'static str>)> {
    let s = params_slice.trim();
    if s.is_empty() {
        return Some((vec![], vec![]));
    }

    let mut req = Vec::new();
    let mut all = Vec::new();

    // naive split by ',' (good enough for simple d.ts)
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // param: `name?: number` or `name: string`
        let colon = part.find(':')?;
        let name_part = part[..colon].trim();
        let is_optional = name_part.ends_with('?');

        let ty = part[colon + 1..].trim();
        let ty_tok = take_ident(ty);
        let mapped = match ty_tok {
            "string" => "string",
            "number" => "f64",
            _ => return None,
        };

        all.push(mapped);
        if !is_optional {
            req.push(mapped);
        }
    }

    Some((req, all))
}

fn take_ident(s: &str) -> &str {
    let mut end = 0;
    for (i, ch) in s.char_indices() {
        if ch.is_alphanumeric() || ch == '_' {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    &s[..end]
}

fn map_ret_token(tok: &str, decl_slice: &str, close_paren: Option<usize>) -> Option<&'static str> {
    match tok {
        "string" => Some("string"),
        "number" => Some("f64"),
        "void" => Some("void"),
        other => {
            // Heuristic: `function nanoid<Type extends string>(...): Type`
            // Treat generic string-like return as string.
            if let Some(cp) = close_paren {
                let decl_head = &decl_slice[..cp];
                if decl_head.contains("extends string") {
                    if other.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                        return Some("string");
                    }
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_export_function_overload() {
        let dts = r#"
export function foo(): string
export function foo(x: number): number
"#;
        assert_eq!(infer_fn_sig(dts, "foo"), Some((vec![], "string")));
    }

    #[test]
    fn infer_export_function_params_optional_number() {
        let dts = r#"export function nanoid(size?: number): string"#;
        // optional param is not required
        assert_eq!(infer_fn_sig(dts, "nanoid"), Some((vec![], "string")));
        // but the full signature includes the optional param
        assert_eq!(
            infer_fn_sig_with_optional(dts, "nanoid"),
            Some((vec![], vec!["f64"], "string"))
        );
    }

    #[test]
    fn infer_export_const_fn() {
        let dts = r#"export const bar: (x: string) => void"#;
        assert_eq!(infer_fn_sig(dts, "bar"), Some((vec!["string"], "void")));
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
