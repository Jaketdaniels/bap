//! Manifest types and independent Rust-side validation of producer evidence.

use crate::proof::{
    AMBIENT_SOURCE, AMBIENT_SPECIFIER, MAX_EVIDENCE_DEPTH, MAX_EVIDENCE_NODES, PROTOCOL_VERSION,
    TYPESCRIPT_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

/// Structural or integrity failure; the manifest cannot be trusted.
pub const CODE_INVALID_MANIFEST: &str = "BAP_PROOF_INVALID_MANIFEST";
/// Evidence violates unit rules, limits, or exact-bit recomputation.
pub const CODE_INVALID_EVIDENCE: &str = "BAP_PROOF_INVALID_EVIDENCE";
/// The folded literal cannot fit inside the original slice width.
pub const CODE_NO_SPACE: &str = "BAP_PROOF_NO_SPACE";

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofRejection {
    code: String,
    message: String,
}

impl ProofRejection {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ProofRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProofRejection {}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofDependency {
    pub specifier: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofDiagnostic {
    pub code: i64,
    pub category: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceNode {
    pub kind: String,
    pub unit: String,
    pub value_bits: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_end: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operand: Option<Box<EvidenceNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Box<EvidenceNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Box<EvidenceNode>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofEntry {
    pub start: usize,
    pub end: usize,
    pub slice_hash: String,
    pub evidence: EvidenceNode,
    pub unit: String,
    pub value: f64,
    pub value_bits: String,
}

/// The document the proof tool writes; unknown fields are rejected.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofManifest {
    pub protocol_version: u32,
    pub typescript_version: String,
    pub specifier: String,
    pub source_hash: String,
    pub dependencies: Vec<ProofDependency>,
    pub diagnostics: Vec<ProofDiagnostic>,
    pub proofs: Vec<ProofEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Value>,
}

/// Independent verdict on a manifest against its source.
#[derive(Debug)]
pub enum ProofAssessment {
    /// Validated entries ready for specialization, sorted by start offset.
    Proven(Vec<ProofEntry>),
    /// TypeScript reported errors; `diagnostic` is e.g. `TS2345`.
    Failed { diagnostic: String, message: String },
    /// Clean run with nothing provable, or a producer-reported failure reason.
    Unproven { reason: String },
}

const KIND_LITERAL: &str = "numeric.literal";
const KIND_POSITIVE: &str = "scalar.positive";
const KIND_NEGATIVE: &str = "scalar.negative";
const KIND_PARENTHESIZED: &str = "scalar.parenthesized";
const KIND_SCALAR_ADD: &str = "scalar.add";
const KIND_SCALAR_SUB: &str = "scalar.sub";
const KIND_SCALAR_MUL: &str = "scalar.mul";
const KIND_SCALAR_DIV: &str = "scalar.div";
const KIND_CONST_REFERENCE: &str = "const.reference";
const KIND_UNIT_CONSTRUCT: &str = "unit.construct";
const KIND_UNIT_ADD: &str = "unit.add";
const KIND_UNIT_SUB: &str = "unit.sub";
const KIND_UNIT_MUL: &str = "unit.mul";
const KIND_UNIT_DIV: &str = "unit.div";

const DIMENSIONLESS: &str = "dimensionless";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_bits(bits: &str) -> Result<u64, ProofRejection> {
    let hex = bits
        .strip_prefix("0x")
        .ok_or_else(|| invalid_evidence(format!("malformed bits {bits:?}")))?;
    if hex.len() != 16 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_evidence(format!("malformed bits {bits:?}")));
    }
    u64::from_str_radix(hex, 16).map_err(|_| invalid_evidence(format!("malformed bits {bits:?}")))
}

fn invalid_manifest(message: impl Into<String>) -> ProofRejection {
    ProofRejection::new(CODE_INVALID_MANIFEST, message)
}

fn invalid_evidence(message: impl Into<String>) -> ProofRejection {
    ProofRejection::new(CODE_INVALID_EVIDENCE, message)
}

/// Validates envelope, hashes, ranges, slices, and every evidence tree from scratch.
pub fn assess(
    manifest: &ProofManifest,
    specifier: &str,
    source: &str,
) -> Result<ProofAssessment, ProofRejection> {
    if manifest.protocol_version != PROTOCOL_VERSION {
        return Err(invalid_manifest(format!(
            "protocol version {} is not {PROTOCOL_VERSION}",
            manifest.protocol_version
        )));
    }
    if manifest.typescript_version != TYPESCRIPT_VERSION {
        return Err(invalid_manifest(format!(
            "typescript version {} is not {TYPESCRIPT_VERSION}",
            manifest.typescript_version
        )));
    }
    if manifest.specifier != specifier {
        return Err(invalid_manifest(format!(
            "specifier {} does not match {specifier}",
            manifest.specifier
        )));
    }
    if manifest.source_hash != digest(source.as_bytes()) {
        return Err(invalid_manifest("source hash does not match the module"));
    }
    if manifest.dependencies.len() != 1
        || manifest.dependencies[0].specifier != AMBIENT_SPECIFIER
        || manifest.dependencies[0].source_hash != digest(AMBIENT_SOURCE.as_bytes())
    {
        return Err(invalid_manifest(
            "dependency set does not match the bundled ambient declaration",
        ));
    }

    if let Some(diagnostic) = manifest
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.category == "error")
    {
        return Ok(ProofAssessment::Failed {
            diagnostic: format!("TS{}", diagnostic.code),
            message: diagnostic.message.clone(),
        });
    }
    if !manifest.proofs.is_empty() && !manifest.diagnostics.is_empty() {
        return Err(invalid_manifest("proven modules must carry no diagnostics"));
    }
    if let Some(failure) = &manifest.failure {
        let code = failure
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let message = failure
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("producer reported a failure without a message");
        return Ok(ProofAssessment::Unproven {
            reason: format!("{code}: {message}"),
        });
    }
    if manifest.proofs.is_empty() {
        return Ok(ProofAssessment::Unproven {
            reason: "no proofable declarations".to_owned(),
        });
    }

    validate_ranges(manifest, source)?;
    validate_evidence(manifest)?;
    Ok(ProofAssessment::Proven(manifest.proofs.clone()))
}

/// Ranges must be in bounds, strictly ascending, non-overlapping, and slice-hashed.
fn validate_ranges(manifest: &ProofManifest, source: &str) -> Result<(), ProofRejection> {
    let mut previous_end = 0usize;
    for entry in &manifest.proofs {
        if entry.start >= entry.end || entry.end > source.len() {
            return Err(invalid_manifest(format!(
                "proof range {}..{} is outside the {}-byte module",
                entry.start,
                entry.end,
                source.len()
            )));
        }
        if entry.start < previous_end {
            return Err(invalid_manifest(format!(
                "proof range {}..{} overlaps or precedes an earlier range",
                entry.start, entry.end
            )));
        }
        previous_end = entry.end;
        let slice = &source.as_bytes()[entry.start..entry.end];
        if entry.slice_hash != digest(slice) {
            return Err(invalid_manifest(format!(
                "slice hash mismatch for range {}..{}",
                entry.start, entry.end
            )));
        }
    }
    Ok(())
}

struct EvidenceValidator<'a> {
    by_range: HashMap<(usize, usize), &'a ProofEntry>,
}

impl<'a> EvidenceValidator<'a> {
    fn new(manifest: &'a ProofManifest) -> Self {
        let by_range = manifest
            .proofs
            .iter()
            .map(|entry| ((entry.start, entry.end), entry))
            .collect();
        Self { by_range }
    }

    fn evaluate(
        &self,
        node: &'a EvidenceNode,
        depth: usize,
        budget: &mut usize,
    ) -> Result<f64, ProofRejection> {
        if depth > MAX_EVIDENCE_DEPTH {
            return Err(invalid_evidence(format!(
                "evidence tree exceeds depth {MAX_EVIDENCE_DEPTH}"
            )));
        }
        *budget += 1;
        if *budget > MAX_EVIDENCE_NODES {
            return Err(invalid_evidence(format!(
                "evidence tree exceeds {MAX_EVIDENCE_NODES} nodes"
            )));
        }
        let declared_bits = parse_bits(&node.value_bits)?;

        let child = |operand: &'a Option<Box<EvidenceNode>>,
                     label: &str|
         -> Result<&'a EvidenceNode, ProofRejection> {
            operand
                .as_deref()
                .ok_or_else(|| invalid_evidence(format!("{} requires {}", node.kind, label)))
        };

        match node.kind.as_str() {
            KIND_LITERAL => {
                self.require_unit(node, DIMENSIONLESS)?;
            }
            KIND_POSITIVE | KIND_NEGATIVE | KIND_PARENTHESIZED => {
                let operand = child(&node.operand, "an operand")?;
                let value = self.evaluate(operand, depth + 1, budget)?;
                self.require_unit(operand, DIMENSIONLESS)?;
                self.require_unit(node, DIMENSIONLESS)?;
                let computed = match node.kind.as_str() {
                    KIND_NEGATIVE => -value,
                    _ => value,
                };
                if computed.to_bits() != declared_bits {
                    return Err(self.bits_mismatch(node, computed));
                }
            }
            KIND_SCALAR_ADD | KIND_SCALAR_SUB | KIND_SCALAR_MUL | KIND_SCALAR_DIV => {
                let left = child(&node.left, "a left operand")?;
                let right = child(&node.right, "a right operand")?;
                let left_value = self.evaluate(left, depth + 1, budget)?;
                let right_value = self.evaluate(right, depth + 1, budget)?;
                self.require_unit(left, DIMENSIONLESS)?;
                self.require_unit(right, DIMENSIONLESS)?;
                self.require_unit(node, DIMENSIONLESS)?;
                self.require_divisor(node.kind.as_str(), right_value)?;
                let computed = apply_scalar(&node.kind, left_value, right_value)?;
                if computed.to_bits() != declared_bits {
                    return Err(self.bits_mismatch(node, computed));
                }
            }
            KIND_CONST_REFERENCE => {
                let name = node.name.as_deref().ok_or_else(|| {
                    invalid_evidence("const.reference requires a name".to_owned())
                })?;
                if name.is_empty() {
                    return Err(invalid_evidence("const name must be non-empty"));
                }
                let target_start = node.target_start.ok_or_else(|| {
                    invalid_evidence("const.reference requires targetStart".to_owned())
                })?;
                let target_end = node.target_end.ok_or_else(|| {
                    invalid_evidence("const.reference requires targetEnd".to_owned())
                })?;
                let target = self
                    .by_range
                    .get(&(target_start, target_end))
                    .ok_or_else(|| {
                        invalid_evidence(format!(
                        "const reference {name} targets unproven range {target_start}..{target_end}"
                    ))
                    })?;
                let operand = child(&node.operand, "an operand")?;
                if *operand != target.evidence {
                    return Err(invalid_evidence(format!(
                        "const reference {name} does not carry the target's evidence"
                    )));
                }
                if node.unit != target.unit || declared_bits != parse_bits(&target.value_bits)? {
                    return Err(invalid_evidence(format!(
                        "const reference {name} disagrees with its target's value or unit"
                    )));
                }
            }
            KIND_UNIT_CONSTRUCT => {
                let name = node
                    .name
                    .as_deref()
                    .ok_or_else(|| invalid_evidence("unit.construct requires a name".to_owned()))?;
                if name.is_empty() || node.unit != name {
                    return Err(invalid_evidence(
                        "unit.construct must declare and carry the same unit",
                    ));
                }
                let operand = child(&node.operand, "an operand")?;
                let value = self.evaluate(operand, depth + 1, budget)?;
                self.require_unit(operand, DIMENSIONLESS)?;
                if value.to_bits() != declared_bits {
                    return Err(self.bits_mismatch(node, value));
                }
            }
            KIND_UNIT_ADD | KIND_UNIT_SUB | KIND_UNIT_MUL | KIND_UNIT_DIV => {
                let left = child(&node.left, "a left operand")?;
                let right = child(&node.right, "a right operand")?;
                let left_value = self.evaluate(left, depth + 1, budget)?;
                let right_value = self.evaluate(right, depth + 1, budget)?;
                let kind = node.kind.as_str();
                if kind == KIND_UNIT_ADD || kind == KIND_UNIT_SUB {
                    if left.unit != right.unit || node.unit != left.unit {
                        return Err(invalid_evidence(format!(
                            "{kind} requires identical operand units carried through"
                        )));
                    }
                } else {
                    self.require_unit(left, DIMENSIONLESS)?;
                    self.require_unit(right, DIMENSIONLESS)?;
                    self.require_unit(node, DIMENSIONLESS)?;
                }
                self.require_divisor(kind, right_value)?;
                let computed = apply_scalar(kind, left_value, right_value)?;
                if computed.to_bits() != declared_bits {
                    return Err(self.bits_mismatch(node, computed));
                }
            }
            other => {
                return Err(invalid_evidence(format!("unknown evidence kind {other:?}")));
            }
        }
        let value = f64::from_bits(declared_bits);
        if !value.is_finite() {
            return Err(invalid_evidence(format!(
                "{} carries a non-finite value",
                node.kind
            )));
        }
        Ok(value)
    }

    fn require_unit(&self, node: &EvidenceNode, expected: &str) -> Result<(), ProofRejection> {
        if node.unit != expected {
            return Err(invalid_evidence(format!(
                "{} must carry unit {expected:?}, found {:?}",
                node.kind, node.unit
            )));
        }
        Ok(())
    }

    fn require_divisor(&self, kind: &str, divisor: f64) -> Result<(), ProofRejection> {
        if (kind == KIND_SCALAR_DIV || kind == KIND_UNIT_DIV) && divisor == 0.0 {
            return Err(invalid_evidence(format!("{kind} divides by zero")));
        }
        Ok(())
    }

    fn bits_mismatch(&self, node: &EvidenceNode, computed: f64) -> ProofRejection {
        invalid_evidence(format!(
            "{} declares bits {}, but operands recompute to {:?}",
            node.kind, node.value_bits, computed
        ))
    }
}

fn apply_scalar(kind: &str, left: f64, right: f64) -> Result<f64, ProofRejection> {
    let value = match kind {
        KIND_SCALAR_ADD | KIND_UNIT_ADD => left + right,
        KIND_SCALAR_SUB | KIND_UNIT_SUB => left - right,
        KIND_SCALAR_MUL | KIND_UNIT_MUL => left * right,
        KIND_SCALAR_DIV | KIND_UNIT_DIV => left / right,
        other => {
            return Err(invalid_evidence(format!(
                "unknown arithmetic kind {other:?}"
            )));
        }
    };
    Ok(value)
}

fn validate_evidence(manifest: &ProofManifest) -> Result<(), ProofRejection> {
    let validator = EvidenceValidator::new(manifest);
    for entry in &manifest.proofs {
        let mut budget = 0usize;
        let value = validator.evaluate(&entry.evidence, 1, &mut budget)?;
        if entry.unit != entry.evidence.unit {
            return Err(invalid_evidence(format!(
                "proof at {}..{} declares unit {:?} over evidence unit {:?}",
                entry.start, entry.end, entry.unit, entry.evidence.unit
            )));
        }
        if entry.value_bits != entry.evidence.value_bits || entry.value.to_bits() != value.to_bits()
        {
            return Err(invalid_evidence(format!(
                "proof at {}..{} disagrees with recomputed evidence bits",
                entry.start, entry.end
            )));
        }
    }
    Ok(())
}
