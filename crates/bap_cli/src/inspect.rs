//! `bap inspect proofs`: run the proof tool and report its validated outcome.

use crate::cli::InspectCommand;
use bap_ts::{ProofAssessment, ProofBackend, proof::file_url_specifier, proof::manifest::assess};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;

/// Handles `bap inspect ...` and returns the process exit code.
pub fn inspect(command: InspectCommand) -> i32 {
    match command {
        InspectCommand::Proofs { file, json } => inspect_proofs(&file, json),
    }
}

fn inspect_proofs(file: &str, json_output: bool) -> i32 {
    let extension = Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !matches!(extension, "ts" | "mts") {
        report(
            json_output,
            json!({
                "file": file,
                "status": "not-applicable",
                "detail": "proofs apply to TypeScript modules (.ts/.mts)",
            }),
            "not applicable: proofs apply to TypeScript modules (.ts/.mts)",
        );
        return 1;
    }
    let source = match std::fs::read_to_string(file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("bap: failed to read {file}: {error}");
            return 1;
        }
    };
    let Some(specifier) = file_url_specifier(Path::new(file)) else {
        eprintln!("bap: failed to canonicalize {file}");
        return 1;
    };

    let Some(backend) = ProofBackend::discover() else {
        report(
            json_output,
            json!({
                "file": file,
                "status": "unavailable",
                "detail": NO_BACKEND_MESSAGE,
            }),
            &format!("unavailable: {NO_BACKEND_MESSAGE}"),
        );
        return 1;
    };

    let manifest = match backend.prove(&source, &specifier) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("bap: proof backend failed: {error:#}");
            return 1;
        }
    };

    match assess(&manifest, &specifier, &source) {
        Ok(ProofAssessment::Proven(entries)) => {
            let mut units = BTreeSet::new();
            let mut kinds = BTreeSet::new();
            for entry in &entries {
                units.insert(entry.unit.clone());
                kinds.insert(entry.evidence.kind.clone());
            }
            report(
                json_output,
                json!({
                    "file": file,
                    "specifier": specifier,
                    "status": "proven",
                    "proofCount": entries.len(),
                    "units": units.iter().cloned().collect::<Vec<_>>(),
                    "kinds": kinds.iter().cloned().collect::<Vec<_>>(),
                }),
                &format!(
                    "proven: {} proof(s), units {:?}, kinds {:?}",
                    entries.len(),
                    units.into_iter().collect::<Vec<_>>(),
                    kinds.into_iter().collect::<Vec<_>>()
                ),
            );
            0
        }
        Ok(ProofAssessment::Failed {
            diagnostic,
            message,
        }) => {
            report(
                json_output,
                json!({
                    "file": file,
                    "specifier": specifier,
                    "status": "failed",
                    "diagnostic": diagnostic,
                    "message": message,
                }),
                &format!("failed: {diagnostic}: {message}"),
            );
            1
        }
        Ok(ProofAssessment::Unproven { reason }) => {
            report(
                json_output,
                json!({
                    "file": file,
                    "specifier": specifier,
                    "status": "unproven",
                    "reason": reason,
                }),
                &format!("unproven: {reason}"),
            );
            0
        }
        Err(rejection) => {
            report(
                json_output,
                json!({
                    "file": file,
                    "specifier": specifier,
                    "status": "rejected",
                    "code": rejection.code(),
                    "message": rejection.message(),
                }),
                &format!("rejected ({}): {}", rejection.code(), rejection.message()),
            );
            1
        }
    }
}

const NO_BACKEND_MESSAGE: &str =
    "no Node proof tool found; set BAP_TS_PROOF_TOOL to tools/ts-proof/proof.mjs";

fn report(json_output: bool, payload: serde_json::Value, text: &str) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("inspect json")
        );
    } else {
        println!("{text}");
    }
}
