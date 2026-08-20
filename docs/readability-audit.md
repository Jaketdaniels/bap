# Readability audit

## Result

The runtime code is readable enough for a proof of concept. Public Rust and
JavaScript comments stay short, names match the module they describe, and the CLI
keeps its command surface small.

The audit checked source comments, command help, the root README, and the roadmap.
It found one misleading user-facing phrase and a few documentation risks.

## Findings

### Fixed

- CLI help called the runtime “TypeScript-native” and claimed “auditable numerics.”
  The command now says it runs JavaScript and TypeScript files.
- The README and roadmap now separate shipped behavior from planned work.
- The project practice page no longer describes a large governance program that the
  proof of concept does not run.

### Keep watching

- The old phase plans contain historical proposals and unchecked examples. Read the
  current README and roadmap first.
- Numeric Rust types exist in `bap_math`; JavaScript numeric bindings do not exist
  yet. Documentation must keep that boundary visible.
- `tools/ts-proof` emits evidence. Bap does not consume that evidence yet, so the
  proof tool is a developer tool rather than part of `bap run`.

## Writing rules

- Use plain English and name the command or file involved.
- Keep Rust and JSDoc comments to two lines when possible.
- Describe measured results with the command and environment that produced them.
- Do not use “native,” “auditable,” “production-ready,” or “guaranteed” unless the
  code and a test support the word.
