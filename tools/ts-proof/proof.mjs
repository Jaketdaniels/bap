import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { version as typescriptVersion } from "typescript";
import { API, DiagnosticCategory, SymbolFlags } from "typescript/unstable/sync";
import { NodeFlags, SyntaxKind } from "typescript/unstable/ast";
import {
  isBinaryExpression,
  isCallExpression,
  isElementAccessExpression,
  isIdentifier,
  isNumericLiteral,
  isParenthesizedExpression,
  isPrefixUnaryExpression,
  isPropertyAccessExpression,
  isStringLiteral,
  isVariableStatement,
} from "typescript/unstable/ast/is";

const PROTOCOL_VERSION = 1;
const MAX_DEPTH = 64;
const MAX_EVIDENCE_NODES = 256;
const MAX_PROOFS = 1024;
const DYNAMIC_GLOBALS = new Set(["Function", "eval", "global", "globalThis", "self", "window"]);
const AMBIENT_SPECIFIER = "bap:units.d.ts";
const AMBIENT_FILE = fileURLToPath(new URL("./bap-units.d.ts", import.meta.url));
const AMBIENT_SOURCE = await readFile(AMBIENT_FILE, "utf8");

class UnsupportedExpression extends Error {}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function canonicalizeSpecifier(specifier) {
  try {
    return new URL(specifier).href;
  } catch {
    return pathToFileURL(resolve(specifier)).href;
  }
}

function byteOffset(source, utf16Offset) {
  return Buffer.byteLength(source.slice(0, utf16Offset), "utf8");
}

function byteRange(source, node, sourceFile) {
  return {
    start: byteOffset(source, node.getStart(sourceFile)),
    end: byteOffset(source, node.getEnd()),
  };
}

function f64Bits(value) {
  const bytes = Buffer.allocUnsafe(8);
  bytes.writeDoubleBE(value);
  return `0x${bytes.toString("hex")}`;
}

function finite(value) {
  if (!Number.isFinite(value)) {
    throw new UnsupportedExpression();
  }
  return value;
}

async function createCheckedProject(source) {
  const directory = await mkdtemp(join(tmpdir(), "bap-ts-proof-project-"));
  const entryPath = join(directory, "entry.ts");
  const ambientPath = join(directory, "bap-units.d.ts");
  const configPath = join(directory, "tsconfig.json");
  const config = {
    compilerOptions: {
      strict: true,
      noEmit: true,
      target: "ESNext",
      module: "ESNext",
      moduleResolution: "Bundler",
      skipLibCheck: false,
    },
    files: ["entry.ts", "bap-units.d.ts"],
  };
  await Promise.all([
    writeFile(entryPath, source, "utf8"),
    writeFile(ambientPath, AMBIENT_SOURCE, "utf8"),
    writeFile(configPath, JSON.stringify(config), "utf8"),
  ]);

  const api = new API({ cwd: directory });
  try {
    const snapshot = api.updateSnapshot({ openProjects: ["tsconfig.json"] });
    const project = snapshot.getProjects()[0];
    if (!project) throw new Error("TypeScript did not open the proof project");
    const sourceFile = project.program.getSourceFile("entry.ts");
    const ambientFile = project.program.getSourceFile("bap-units.d.ts");
    if (!sourceFile || !ambientFile) throw new Error("TypeScript omitted a proof input");
    return {
      api,
      directory,
      project,
      sourceFile,
      ambientFile,
    };
  } catch (error) {
    api.close();
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
}

function formatDiagnostic(diagnostic, source, sourceFile, specifier) {
  const result = {
    code: diagnostic.code,
    category: DiagnosticCategory[diagnostic.category].toLowerCase(),
    message: diagnostic.text,
  };
  if (diagnostic.fileName === sourceFile.fileName && diagnostic.pos >= 0) {
    const location = sourceFile.getLineAndCharacterOfPosition(diagnostic.pos);
    result.specifier = specifier;
    result.start = byteOffset(source, diagnostic.pos);
    result.end = byteOffset(source, diagnostic.end);
    result.line = location.line + 1;
    result.column = location.character + 1;
  }
  return result;
}

function isTopLevelConst(statement) {
  return isVariableStatement(statement)
    && (statement.declarationList.flags & NodeFlags.Const) !== 0;
}

function declaredOnlyInAmbient(symbol, ambientFile, project) {
  return Boolean(symbol?.declarations?.length)
    && symbol.declarations.every(
      (handle) => handle.resolve(project)?.getSourceFile().fileName === ambientFile.fileName,
    );
}

function isAmbientBapIdentifier(node, checker, ambientFile, project) {
  return isIdentifier(node)
    && node.text === "Bap"
    && declaredOnlyInAmbient(checker.getSymbolAtLocation(node), ambientFile, project);
}

function isBapNumericAccess(node, checker, ambientFile, project) {
  const propertyAccess = isPropertyAccessExpression(node)
    && node.name.text === "numeric";
  const elementAccess = isElementAccessExpression(node)
    && isStringLiteral(node.argumentExpression)
    && node.argumentExpression.text === "numeric";
  if ((!propertyAccess && !elementAccess)
      || !isAmbientBapIdentifier(node.expression, checker, ambientFile, project)) {
    return false;
  }
  return true;
}

function isDirectTrustedCall(node) {
  const helper = node.parent;
  const call = helper?.parent;
  return isPropertyAccessExpression(node)
    && isPropertyAccessExpression(helper)
    && helper.expression === node
    && !node.questionDotToken
    && !helper.questionDotToken
    && isCallExpression(call)
    && call.expression === helper
    && !call.questionDotToken;
}

function findUnsafeProofCapability(sourceFile, checker, ambientFile, project) {
  let unsafeNode;
  const visit = (node) => {
    if (unsafeNode) return;
    if (isIdentifier(node) && DYNAMIC_GLOBALS.has(node.text)) {
      unsafeNode = node;
      return;
    }
    if (isAmbientBapIdentifier(node, checker, ambientFile, project)) {
      const numericAccess = node.parent;
      if (!isBapNumericAccess(numericAccess, checker, ambientFile, project)
          || !isDirectTrustedCall(numericAccess)) {
        unsafeNode = node;
        return;
      }
    }
    node.forEachChild(visit);
  };
  sourceFile.forEachChild(visit);
  return unsafeNode;
}

function unsafeMutationFailure(source, node, sourceFile, specifier) {
  const range = byteRange(source, node, sourceFile);
  const location = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return {
    code: "BAP_PROOF_UNSAFE_MUTATION",
    message: "proof request contains dynamic or escaped access that can mutate Bap",
    specifier,
    ...range,
    line: location.line + 1,
    column: location.character + 1,
  };
}

function trustedHelper(node, checker, ambientFile, project) {
  if (!isPropertyAccessExpression(node)
      || !isPropertyAccessExpression(node.expression)
      || node.questionDotToken
      || node.expression.questionDotToken
      || !isIdentifier(node.expression.expression)
      || node.expression.expression.text !== "Bap"
      || node.expression.name.text !== "numeric") {
    return undefined;
  }
  for (const part of [node.expression.expression, node.expression.name, node.name]) {
    const symbol = checker.getSymbolAtLocation(part);
    if (!declaredOnlyInAmbient(symbol, ambientFile, project)) {
      return undefined;
    }
  }
  return node.name.text;
}

function evaluator(source, sourceFile, ambientFile, project, checker, declarations) {
  function evaluate(node, context, depth = 1) {
    if (depth > MAX_DEPTH) throw new UnsupportedExpression();

    const finish = (value, unit, evidence) => {
      context.nodes += 1;
      if (context.nodes > MAX_EVIDENCE_NODES) throw new UnsupportedExpression();
      finite(value);
      return {
        value,
        unit,
        evidence: { ...evidence, unit, valueBits: f64Bits(value) },
      };
    };

    if (isNumericLiteral(node)) {
      const value = finite(Number(node.text));
      return finish(value, "dimensionless", { kind: "numeric.literal" });
    }

    if (isPrefixUnaryExpression(node)
        && (node.operator === SyntaxKind.PlusToken || node.operator === SyntaxKind.MinusToken)) {
      const operand = evaluate(node.operand, context, depth + 1);
      if (operand.unit !== "dimensionless") throw new UnsupportedExpression();
      const value = node.operator === SyntaxKind.MinusToken ? -operand.value : +operand.value;
      const kind = node.operator === SyntaxKind.MinusToken ? "scalar.negative" : "scalar.positive";
      return finish(value, "dimensionless", { kind, operand: operand.evidence });
    }

    if (isParenthesizedExpression(node)) {
      const operand = evaluate(node.expression, context, depth + 1);
      return finish(operand.value, operand.unit, {
        kind: "scalar.parenthesized",
        operand: operand.evidence,
      });
    }

    if (isBinaryExpression(node)) {
      const kinds = new Map([
        [SyntaxKind.PlusToken, "scalar.add"],
        [SyntaxKind.MinusToken, "scalar.sub"],
        [SyntaxKind.AsteriskToken, "scalar.mul"],
        [SyntaxKind.SlashToken, "scalar.div"],
      ]);
      const kind = kinds.get(node.operatorToken.kind);
      if (!kind) throw new UnsupportedExpression();
      const left = evaluate(node.left, context, depth + 1);
      const right = evaluate(node.right, context, depth + 1);
      if (left.unit !== "dimensionless" || right.unit !== "dimensionless") {
        throw new UnsupportedExpression();
      }
      if (kind === "scalar.div" && right.value === 0) throw new UnsupportedExpression();
      const operations = {
        "scalar.add": () => left.value + right.value,
        "scalar.sub": () => left.value - right.value,
        "scalar.mul": () => left.value * right.value,
        "scalar.div": () => left.value / right.value,
      };
      return finish(operations[kind](), "dimensionless", {
        kind,
        left: left.evidence,
        right: right.evidence,
      });
    }

    if (isIdentifier(node)) {
      let symbol = checker.getSymbolAtLocation(node);
      if (symbol && (symbol.flags & SymbolFlags.Alias) !== 0) {
        symbol = checker.getAliasedSymbol(symbol);
      }
      const declaration = declarations.get(symbol);
      if (!declaration?.initializer || context.active.has(declaration)) {
        throw new UnsupportedExpression();
      }
      context.active.add(declaration);
      try {
        const target = evaluate(declaration.initializer, context, depth + 1);
        const range = byteRange(source, declaration.initializer, sourceFile);
        return finish(target.value, target.unit, {
          kind: "const.reference",
          name: node.text,
          targetStart: range.start,
          targetEnd: range.end,
          operand: target.evidence,
        });
      } finally {
        context.active.delete(declaration);
      }
    }

    if (isCallExpression(node)) {
      if (node.questionDotToken) throw new UnsupportedExpression();
      const helper = trustedHelper(node.expression, checker, ambientFile, project);
      if (!helper) throw new UnsupportedExpression();
      if (helper === "unit") {
        if (node.arguments.length !== 2 || !isStringLiteral(node.arguments[0])) {
          throw new UnsupportedExpression();
        }
        const operand = evaluate(node.arguments[1], context, depth + 1);
        if (operand.unit !== "dimensionless") throw new UnsupportedExpression();
        return finish(operand.value, node.arguments[0].text, {
          kind: "unit.construct",
          name: node.arguments[0].text,
          operand: operand.evidence,
        });
      }

      const helperKinds = {
        unitAdd: "unit.add",
        unitSub: "unit.sub",
        unitMul: "unit.mul",
        unitDiv: "unit.div",
      };
      const kind = helperKinds[helper];
      if (!kind || node.arguments.length !== 2) throw new UnsupportedExpression();
      const left = evaluate(node.arguments[0], context, depth + 1);
      const right = evaluate(node.arguments[1], context, depth + 1);
      if ((kind === "unit.add" || kind === "unit.sub") && left.unit !== right.unit) {
        throw new UnsupportedExpression();
      }
      if ((kind === "unit.mul" || kind === "unit.div")
          && (left.unit !== "dimensionless" || right.unit !== "dimensionless")) {
        throw new UnsupportedExpression();
      }
      if (kind === "unit.div" && right.value === 0) throw new UnsupportedExpression();
      const operations = {
        "unit.add": () => left.value + right.value,
        "unit.sub": () => left.value - right.value,
        "unit.mul": () => left.value * right.value,
        "unit.div": () => left.value / right.value,
      };
      const unit = kind === "unit.add" || kind === "unit.sub" ? left.unit : "dimensionless";
      return finish(operations[kind](), unit, {
        kind,
        left: left.evidence,
        right: right.evidence,
      });
    }

    throw new UnsupportedExpression();
  }

  return evaluate;
}

export async function prove(source, options = {}) {
  if (typeof source !== "string") throw new TypeError("source must be a string");
  const specifier = canonicalizeSpecifier(options.specifier ?? "file:///entry.ts");
  const checked = await createCheckedProject(source);
  try {
    const { project, sourceFile, ambientFile } = checked;
    const program = project.program;
    const checker = project.checker;
    const rawDiagnostics = [
      ...program.getProgramDiagnostics(),
      ...program.getConfigFileParsingDiagnostics(),
      ...program.getSyntacticDiagnostics(),
      ...program.getBindDiagnostics(),
      ...program.getGlobalDiagnostics(),
      ...program.getSemanticDiagnostics(),
      ...program.getDeclarationDiagnostics(),
    ];
    const diagnostics = rawDiagnostics
      .map((diagnostic) => formatDiagnostic(diagnostic, source, sourceFile, specifier));
    const manifest = {
      protocolVersion: PROTOCOL_VERSION,
      typescriptVersion,
      specifier,
      sourceHash: sha256(Buffer.from(source, "utf8")),
      dependencies: [{
        specifier: AMBIENT_SPECIFIER,
        sourceHash: sha256(Buffer.from(AMBIENT_SOURCE, "utf8")),
      }],
      diagnostics,
      proofs: [],
    };
    const unsafeNode = findUnsafeProofCapability(sourceFile, checker, ambientFile, project);
    if (unsafeNode) {
      manifest.failure = unsafeMutationFailure(source, unsafeNode, sourceFile, specifier);
      return manifest;
    }
    if (diagnostics.some(({ category }) => category === "error")) return manifest;

    const declarations = new Map();
    const candidates = [];
    for (const statement of sourceFile.statements) {
      if (!isTopLevelConst(statement)) continue;
      for (const declaration of statement.declarationList.declarations) {
        if (!isIdentifier(declaration.name) || !declaration.initializer) continue;
        const symbol = checker.getSymbolAtLocation(declaration.name);
        if (symbol) declarations.set(symbol, declaration);
        candidates.push(declaration);
      }
    }

    const evaluate = evaluator(source, sourceFile, ambientFile, project, checker, declarations);
    const eligibleProofs = [];
    for (const declaration of candidates) {
      try {
        const context = { nodes: 0, active: new Set([declaration]) };
        const result = evaluate(declaration.initializer, context);
        const range = byteRange(source, declaration.initializer, sourceFile);
        const slice = Buffer.from(source, "utf8").subarray(range.start, range.end);
        eligibleProofs.push({
          ...range,
          sliceHash: sha256(slice),
          evidence: result.evidence,
          unit: result.unit,
          value: result.value,
          valueBits: f64Bits(result.value),
        });
      } catch (error) {
        if (!(error instanceof UnsupportedExpression)) throw error;
      }
    }
    if (eligibleProofs.length > MAX_PROOFS) {
      manifest.failure = {
        code: "BAP_PROOF_EDIT_LIMIT",
        message: `proof request has ${eligibleProofs.length} eligible edits; limit is ${MAX_PROOFS}`,
        limit: MAX_PROOFS,
        eligibleEdits: eligibleProofs.length,
      };
      return manifest;
    }
    manifest.proofs = eligibleProofs;
    return manifest;
  } finally {
    checked.api.close();
    await rm(checked.directory, { recursive: true, force: true });
  }
}

async function runCommand() {
  const [requestPath, responsePath] = process.argv.slice(2);
  if (!requestPath || !responsePath) {
    throw new Error("usage: node proof.mjs <request.json> <response.json>");
  }
  const request = JSON.parse(await readFile(requestPath, "utf8"));
  const manifest = await prove(request.source, { specifier: request.specifier });
  await writeFile(responsePath, `${JSON.stringify(manifest)}\n`, "utf8");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCommand().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
