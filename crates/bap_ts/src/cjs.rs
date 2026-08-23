//! CommonJS detection and ESM wrapper synthesis.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How a module's source should be executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    /// Standard ES module syntax.
    Esm,
    /// CommonJS `require`/`module.exports`; wrapped into a synthetic ESM.
    CommonJs,
}

/// Names that cannot appear in `export { ... }` lists.
const RESERVED_EXPORTS: &[&str] = &[
    "default",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "let",
    "static",
];

const MAX_NAMED_EXPORTS: usize = 128;

/// Decides how to execute the module at `path` with `source`.
#[cfg(test)]
pub fn classify(path: &Path, source: &str) -> ModuleKind {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("cjs") => return ModuleKind::CommonJs,
        Some("mjs" | "mts" | "ts") => return ModuleKind::Esm,
        _ => {}
    }
    if nearest_package_type(path) == Some("module") {
        return ModuleKind::Esm;
    }
    if looks_like_esm(source) {
        ModuleKind::Esm
    } else {
        ModuleKind::CommonJs
    }
}

fn looks_like_esm(source: &str) -> bool {
    for line in source.lines() {
        let trimmed = line.trim_start();
        let after_comments = trimmed.trim_start_matches([';', '}']);
        if after_comments.starts_with("await ")
            || after_comments.starts_with("await(")
            || after_comments.starts_with("import ")
            || after_comments.starts_with("import{")
            || after_comments.starts_with("import*")
            || after_comments.starts_with("import\"")
            || after_comments.starts_with("import'")
            || after_comments.starts_with("export ")
            || after_comments.starts_with("export{")
            || after_comments.starts_with("export default")
            || after_comments.starts_with("export*")
        {
            return true;
        }
    }
    false
}

/// Walks up from `path` looking for the nearest package.json `type` field.
fn nearest_package_type(path: &Path) -> Option<&'static str> {
    let mut directory = path.parent()?;
    loop {
        let manifest = directory.join("package.json");
        if manifest.is_file() {
            let text = std::fs::read_to_string(manifest).ok()?;
            let value: serde_json::Value = serde_json::from_str(&text).ok()?;
            return match value.get("type").and_then(|ty| ty.as_str()) {
                Some("module") => Some("module"),
                Some("commonjs") => Some("commonjs"),
                _ => None,
            };
        }
        directory = directory.parent()?;
    }
}

/// Extracts best-effort named exports (`exports.X =` / `module.exports.X =`).
pub fn detect_named_exports(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for pattern in ["exports.", "module.exports."] {
        let mut cursor = 0usize;
        while let Some(position) = find_from(source, pattern, cursor) {
            let name_start = position + pattern.len();
            let rest = &source[name_start..];
            let name_len = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
                .map(char::len_utf8)
                .sum::<usize>();
            if name_len == 0 {
                cursor = name_start + 1;
                continue;
            }
            cursor = name_start;
            let after = rest[name_len..].trim_start();
            if !after.starts_with('=') || after[1..].starts_with('=') {
                continue;
            }
            let name = &rest[..name_len];
            if is_valid_identifier(name) && !RESERVED_EXPORTS.contains(&name) {
                let fresh = !names.iter().any(|existing| existing == name);
                if fresh {
                    names.push(name.to_owned());
                    if names.len() >= MAX_NAMED_EXPORTS {
                        return names;
                    }
                }
            }
        }
    }
    names
}

fn find_from(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .find(needle)
        .map(|position| position + from)
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Remembers package.json lookups so graphs of many files stay cheap.
#[derive(Default)]
pub struct PackageTypeCache {
    cache: RefCell<HashMap<PathBuf, Option<&'static str>>>,
}

impl PackageTypeCache {
    pub fn classify_cached(
        &self,
        path: &Path,
        source: &str,
        forced: Option<ModuleKind>,
    ) -> ModuleKind {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("cjs") => return ModuleKind::CommonJs,
            Some("mjs" | "mts" | "ts") => return ModuleKind::Esm,
            _ => {}
        }
        if let Some(kind) = forced {
            return kind;
        }
        let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let package_type = *self
            .cache
            .borrow_mut()
            .entry(directory)
            .or_insert_with(|| nearest_package_type(path));
        if package_type == Some("module") {
            return ModuleKind::Esm;
        }
        if looks_like_esm(source) {
            ModuleKind::Esm
        } else {
            ModuleKind::CommonJs
        }
    }
}

/// Builds the synthetic ESM that evaluates a CJS module through `__bapCjs`.
pub fn synthesize_wrapper(specifier_path: &Path, source: &str) -> String {
    let absolute =
        std::fs::canonicalize(specifier_path).unwrap_or_else(|_| specifier_path.to_path_buf());
    let names = detect_named_exports(source);
    let mut out = String::with_capacity(source.len() / 2 + 256);
    out.push_str(&format!(
        "const __bapCjsModule = globalThis.__bapCjs.load({}).exports;\n",
        json_string(&absolute.to_string_lossy())
    ));
    out.push_str("export default __bapCjsModule;\n");
    let mut exported = Vec::new();
    for name in names {
        out.push_str(&format!(
            "const {name} = __bapCjsModule.{};\n",
            property_key(&name)
        ));
        exported.push(name);
    }
    if !exported.is_empty() {
        out.push_str(&format!("export {{ {} }};\n", exported.join(", ")));
    }
    out
}

fn property_key(name: &str) -> String {
    // Identifiers are pre-validated; plain member access is safe.
    name.to_owned()
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("json string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_decides_the_default_kind() {
        assert_eq!(classify(Path::new("/x/a.cjs"), "1;"), ModuleKind::CommonJs);
        assert_eq!(classify(Path::new("/x/a.mjs"), "1;"), ModuleKind::Esm);
        assert_eq!(
            classify(Path::new("/x/a.ts"), "const x = 1;"),
            ModuleKind::Esm
        );
    }

    #[test]
    fn import_and_export_markers_mean_esm() {
        assert_eq!(
            classify(Path::new("/x/a.js"), "const v = await load();\n"),
            ModuleKind::Esm
        );
        assert_eq!(
            classify(Path::new("/x/a.js"), "import { join } from './y.js';\n"),
            ModuleKind::Esm
        );
        assert_eq!(
            classify(Path::new("/x/a.js"), "export const answer = 42;\n"),
            ModuleKind::Esm
        );
        assert_eq!(
            classify(Path::new("/x/a.js"), "const answer = 42;\n"),
            ModuleKind::CommonJs
        );
        // Mentions inside strings on other lines do not fool the marker scan
        // as long as a real require call exists at line start.
        assert_eq!(
            classify(
                Path::new("/x/a.js"),
                "// import lookalike\nconst s = require('fs');\n"
            ),
            ModuleKind::CommonJs
        );
    }

    #[test]
    fn package_json_type_module_wins() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), b"{\"type\": \"module\"}").unwrap();
        let file = dir.path().join("plain.js");
        std::fs::write(&file, b"const answer = 42;\n").unwrap();
        assert_eq!(classify(&file, "const answer = 42;\n"), ModuleKind::Esm);
    }

    #[test]
    fn named_export_detection_covers_both_forms() {
        let source = r#"
exports.add = function add(a, b) { return a + b; };
module.exports.name = 'bap';
const equal = exports.add === add;
exports.multiply = (a) => a * 2;
"#;
        let names = detect_named_exports(source);
        assert!(names.contains(&"add".to_owned()));
        assert!(names.contains(&"name".to_owned()));
        assert!(names.contains(&"multiply".to_owned()));
        // `equal` is only read, never assigned through the pattern.
        assert!(!names.contains(&"equal".to_owned()));
    }

    #[test]
    fn wrapper_loads_by_absolute_path_and_reexports_names() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("local.cjs");
        std::fs::write(&file, b"exports.answer = 42;\n").unwrap();
        let wrapper = synthesize_wrapper(&file, "exports.answer = 42;\n");
        assert!(wrapper.contains("__bapCjs.load("), "{wrapper}");
        assert!(wrapper.contains("export default __bapCjsModule;"));
        assert!(wrapper.contains("const answer = __bapCjsModule.answer;"));
        assert!(wrapper.contains("export { answer };"));
    }
}
