import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";
import { prove } from "../proof.mjs";

const execFileAsync = promisify(execFile);
const root = dirname(dirname(fileURLToPath(import.meta.url)));

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sourceWith(expression, prefix = "") {
  return `${prefix}export const result = ${expression};\n`;
}

test("emits protocol v1 metadata and closed evidence", async () => {
  const source = sourceWith('Bap.numeric.unitAdd(Bap.numeric.unit("metre", 2), Bap.numeric.unit("metre", 3))');
  const result = await prove(source, { specifier: "file:///workspace/../entry.ts" });

  assert.equal(result.protocolVersion, 1);
  assert.equal(result.typescriptVersion, "7.0.2");
  assert.equal(result.specifier, "file:///entry.ts");
  assert.equal(result.sourceHash, sha256(source));
  assert.equal(result.dependencies.length, 1);
  assert.equal(result.dependencies[0].specifier, "bap:units.d.ts");
  assert.match(result.dependencies[0].sourceHash, /^[a-f0-9]{64}$/);
  assert.deepEqual(result.diagnostics, []);
  assert.equal(result.proofs.length, 1);

  const [proof] = result.proofs;
  assert.equal(Buffer.from(source).subarray(proof.start, proof.end).toString(), 'Bap.numeric.unitAdd(Bap.numeric.unit("metre", 2), Bap.numeric.unit("metre", 3))');
  assert.equal(proof.sliceHash, sha256(Buffer.from(source).subarray(proof.start, proof.end)));
  assert.equal(proof.evidence.kind, "unit.add");
  assert.equal(proof.unit, "metre");
  assert.equal(proof.value, 5);
  assert.equal(proof.valueBits, "0x4014000000000000");
});

test("rejects mismatched units with TS2345 and a source location", async () => {
  const source = [
    'const length = Bap.numeric.unit("metre", 2);',
    'const time = Bap.numeric.unit("second", 3);',
    "export const bad = Bap.numeric.unitAdd(length, time);",
    "",
  ].join("\n");
  const result = await prove(source);
  const diagnostic = result.diagnostics.find(({ code }) => code === 2345);

  assert.ok(diagnostic);
  assert.equal(diagnostic.category, "error");
  assert.equal(diagnostic.specifier, "file:///entry.ts");
  assert.equal(diagnostic.line, 3);
  assert.ok(diagnostic.column > 1);
  assert.equal(Buffer.from(source).subarray(diagnostic.start, diagnostic.end).toString(), "length");
  assert.deepEqual(result.proofs, []);
});

test("uses UTF-8 byte ranges and preserves negative zero bits", async () => {
  const source = sourceWith('Bap.numeric.unit("dimensionless", -0)', 'const label = "💡";\n');
  const result = await prove(source);
  const [proof] = result.proofs;

  assert.equal(Buffer.from(source).subarray(proof.start, proof.end).toString(), 'Bap.numeric.unit("dimensionless", -0)');
  assert.equal(proof.valueBits, "0x8000000000000000");
  assert.ok(Object.is(proof.value, -0));
});

test("proves scalar arithmetic and top-level const references", async () => {
  const source = [
    "const base = -(2 + 3) * 4;",
    "export const result = base / 2;",
    "",
  ].join("\n");
  const result = await prove(source);
  const resultProof = result.proofs.find((proof) => Buffer.from(source).subarray(proof.start, proof.end).toString() === "base / 2");

  assert.equal(resultProof.unit, "dimensionless");
  assert.equal(resultProof.value, -10);
  assert.equal(resultProof.evidence.kind, "scalar.div");
  assert.equal(resultProof.evidence.left.kind, "const.reference");
});

test("proves subtraction and dimensionless helper multiplication and division", async () => {
  const source = [
    'const one = Bap.numeric.unit("metre", 8);',
    'const two = Bap.numeric.unit("metre", 3);',
    "export const distance = Bap.numeric.unitSub(one, two);",
    'const a = Bap.numeric.unit("dimensionless", 12);',
    'const b = Bap.numeric.unit("dimensionless", 3);',
    "export const product = Bap.numeric.unitMul(a, b);",
    "export const quotient = Bap.numeric.unitDiv(a, b);",
    "",
  ].join("\n");
  const result = await prove(source);
  const roots = result.proofs.map(({ evidence }) => evidence.kind);

  assert.ok(roots.includes("unit.sub"));
  assert.ok(roots.includes("unit.mul"));
  assert.ok(roots.includes("unit.div"));
  assert.equal(result.proofs.find(({ evidence }) => evidence.kind === "unit.sub").unit, "metre");
});

test("shuns shadowed Bap helpers", async () => {
  const source = [
    "const Bap = { numeric: { unit: (_name: string, value: number) => value } };",
    'export const result = Bap.numeric.unit("metre", 2);',
    "",
  ].join("\n");
  const result = await prove(source);

  assert.deepEqual(result.diagnostics, []);
  assert.deepEqual(result.proofs, []);
});

test("shuns optional access to trusted helpers", async () => {
  const source = sourceWith('Bap?.numeric.unit("metre", 2)');
  const result = await prove(source);

  assert.deepEqual(result.diagnostics, []);
  assert.deepEqual(result.proofs, []);
});

test("shuns cycles, division by zero, and nonfinite results", async () => {
  const cases = [
    "const a = b; const b = a;",
    "export const result = 1 / 0;",
    "export const result = 1e308 * 1e308;",
  ];

  for (const source of cases) {
    const result = await prove(source);
    assert.deepEqual(result.proofs, []);
  }
});

test("shuns unit addition through scalar operators", async () => {
  const source = [
    'const a = Bap.numeric.unit("metre", 2);',
    'const b = Bap.numeric.unit("metre", 3);',
    "export const result = a + b;",
    "",
  ].join("\n");
  const result = await prove(source);

  assert.deepEqual(result.proofs.filter(({ start }) => start > source.indexOf("export")), []);
});

test("enforces expression depth and evidence node limits", async () => {
  const tooDeep = `${"(".repeat(65)}1${")".repeat(65)}`;
  const tooManyNodes = Array.from({ length: 130 }, () => "1").join(" + ");

  assert.deepEqual((await prove(sourceWith(tooDeep))).proofs, []);
  assert.deepEqual((await prove(sourceWith(tooManyNodes))).proofs, []);
});

test("caps output at 1024 sorted non-overlapping edits", async () => {
  const source = Array.from({ length: 1025 }, (_, index) => `export const value${index} = ${index};`).join("\n");
  const result = await prove(source);

  assert.equal(result.proofs.length, 1024);
  assert.ok(result.proofs.every((proof, index, proofs) => index === 0 || proofs[index - 1].end <= proof.start));
});

test("only proves top-level const initializers", async () => {
  const source = [
    "let mutable = 1;",
    "function nested() { const hidden = 2; return hidden; }",
    "const declared: number = 3;",
    "const { destructured } = { destructured: 4 };",
    "",
  ].join("\n");
  const result = await prove(source);

  assert.equal(result.proofs.length, 1);
  assert.equal(Buffer.from(source).subarray(result.proofs[0].start, result.proofs[0].end).toString(), "3");
});

test("rejects helper division by negative zero", async () => {
  const source = sourceWith('Bap.numeric.unitDiv(Bap.numeric.unit("dimensionless", 2), Bap.numeric.unit("dimensionless", -0))');
  const result = await prove(source);

  assert.deepEqual(result.proofs, []);
});

test("ships a schema that requires the protocol contract", async () => {
  const schema = JSON.parse(await readFile(join(root, "manifest.schema.json"), "utf8"));

  assert.equal(schema.$id, "https://bap.dev/schemas/proof-manifest-v1.json");
  assert.deepEqual(schema.required, [
    "protocolVersion",
    "typescriptVersion",
    "specifier",
    "sourceHash",
    "dependencies",
    "diagnostics",
    "proofs",
  ]);
  assert.deepEqual(schema.properties.proofs.maxItems, 1024);
  assert.ok(schema.$defs.evidence.properties.kind.enum.includes("unit.add"));
});

test("supports request and response files as a standalone command", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bap-ts-proof-"));
  const request = join(directory, "request.json");
  const response = join(directory, "response.json");
  try {
    await writeFile(request, JSON.stringify({
      source: sourceWith('Bap.numeric.unit("metre", 7)'),
      specifier: "file:///cli.ts",
    }));
    await execFileAsync(process.execPath, [join(root, "proof.mjs"), request, response]);
    const result = JSON.parse(await readFile(response, "utf8"));

    assert.equal(result.specifier, "file:///cli.ts");
    assert.equal(result.proofs[0].value, 7);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
