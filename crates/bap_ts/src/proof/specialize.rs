//! Width-preserving replacement of proven initializers with folded literals.

use crate::proof::manifest::{CODE_NO_SPACE, ProofEntry, ProofRejection};

/// One prepared replacement; `text` always occupies exactly `end - start` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Converts each proof into a padded literal that round-trips bit-exactly.
pub fn plan_edits(entries: &[ProofEntry], _source: &str) -> Result<Vec<Edit>, ProofRejection> {
    let mut edits = Vec::with_capacity(entries.len());
    for entry in entries {
        let width = entry.end - entry.start;
        let text = format_literal(entry.value_bits.as_str(), width)?;
        edits.push(Edit {
            start: entry.start,
            end: entry.end,
            text,
        });
    }
    Ok(edits)
}

/// Applies edits highest-offset-first so earlier offsets stay valid.
pub fn apply_edits(source: &str, edits: &[Edit]) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut ordered = edits.to_vec();
    ordered.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in &ordered {
        debug_assert!(edit.text.len() == edit.end - edit.start);
        bytes.splice(edit.start..edit.end, edit.text.bytes());
    }
    String::from_utf8(bytes).expect("ASCII replacements keep UTF-8 valid")
}

/// Chooses the shortest bit-exact spelling and left-pads with spaces to `width`.
fn format_literal(bits: &str, width: usize) -> Result<String, ProofRejection> {
    let hex = bits.strip_prefix("0x").ok_or_else(|| {
        ProofRejection::new(
            "BAP_PROOF_INVALID_EVIDENCE",
            format!("malformed bits {bits:?}"),
        )
    })?;
    let raw = u64::from_str_radix(hex, 16).map_err(|_| {
        ProofRejection::new(
            "BAP_PROOF_INVALID_EVIDENCE",
            format!("malformed bits {bits:?}"),
        )
    })?;
    let value = f64::from_bits(raw);
    if !value.is_finite() {
        return Err(ProofRejection::new(
            "BAP_PROOF_INVALID_EVIDENCE",
            "proven value is not finite",
        ));
    }
    if value == 0.0 {
        return pad(if value.is_sign_negative() { "-0" } else { "0" }, width);
    }
    let plain = format!("{}", value);
    if is_exact(&plain, value) {
        return pad(&plain, width);
    }
    let scientific = format!("{:e}", value);
    if is_exact(&scientific, value) {
        return pad(&scientific, width);
    }
    Err(ProofRejection::new(
        CODE_NO_SPACE,
        format!("no bit-exact spelling of {bits} fits in {width} bytes"),
    ))
}

fn is_exact(text: &str, value: f64) -> bool {
    text.parse::<f64>()
        .map(|parsed| parsed.to_bits() == value.to_bits())
        .unwrap_or(false)
}

fn pad(text: &str, width: usize) -> Result<String, ProofRejection> {
    if text.len() > width {
        return Err(ProofRejection::new(
            CODE_NO_SPACE,
            format!(
                "{text:?} needs {} bytes but only {width} are available",
                text.len()
            ),
        ));
    }
    Ok(format!("{}{}", " ".repeat(width - text.len()), text))
}
