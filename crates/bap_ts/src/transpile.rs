use deno_ast::{
    EmitOptions, MediaType, ParseParams, SourceMapOption, TranspileModuleOptions, TranspileOptions,
    parse_module,
};
use deno_core::anyhow::{Context, Result};

/// JavaScript and its separate source map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranspileOutput {
    pub code: String,
    pub source_map: String,
}

/// Transpiles TypeScript modules and leaves JavaScript unchanged.
pub fn transpile(specifier: &str, source: &str) -> Result<TranspileOutput> {
    let specifier =
        deno_ast::ModuleSpecifier::parse(specifier).context("invalid module specifier")?;
    let media_type = MediaType::from_specifier(&specifier);
    if !matches!(media_type, MediaType::TypeScript | MediaType::Mts) {
        return Ok(TranspileOutput {
            code: source.to_owned(),
            source_map: String::new(),
        });
    }

    let parsed = parse_module(ParseParams {
        specifier,
        text: source.into(),
        media_type,
        capture_tokens: false,
        maybe_syntax: None,
        scope_analysis: false,
    })
    .context("failed to parse TypeScript")?;
    let emitted = parsed
        .transpile(
            &TranspileOptions::default(),
            &TranspileModuleOptions::default(),
            &EmitOptions {
                source_map: SourceMapOption::Separate,
                inline_sources: true,
                ..Default::default()
            },
        )
        .context("failed to transpile TypeScript")?
        .into_source();

    Ok(TranspileOutput {
        code: emitted.text,
        source_map: emitted.source_map.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_types_and_preserves_value_imports() {
        let output = transpile(
            "file:///entry.ts",
            "import { x } from './x.ts'; const n: number = x;",
        )
        .unwrap();
        assert!(!output.code.contains(": number"));
        assert!(output.code.contains("from './x.ts'"));
        assert!(output.source_map.contains("entry.ts"));
    }

    #[test]
    fn javascript_is_returned_byte_for_byte() {
        let source = "export const answer = 42;\n";
        let output = transpile("file:///entry.js", source).unwrap();
        assert_eq!(output.code, source);
    }
}
