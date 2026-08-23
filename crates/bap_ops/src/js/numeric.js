const OP_ADD = 0;
const OP_SUB = 1;
const OP_MUL = 2;
const OP_DIV = 3;
const MAX_BATCH_ELEMENTS = 1000000;
const DIMENSIONLESS = "dimensionless";
const INTERVAL_TAG = "__bapInterval";
const UNIT_TAG = "__bapUnit";

const authenticIntervals = new WeakSet();

function invalid(message) {
  return new TypeError(`BAP_NUMERIC_INVALID_ARGUMENT: ${message}`);
}

function toNumber(value, label) {
  if (typeof value !== "number") throw invalid(`${label} must be a number`);
  return value;
}

function toInteger(value, label) {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    throw invalid(`${label} must be an integer`);
  }
  return value;
}

function toBig(value, label) {
  if (typeof value !== "bigint") throw invalid(`${label} must be a bigint`);
  return value;
}

function toText(value, label) {
  if (typeof value !== "string") throw invalid(`${label} must be a string`);
  return value;
}

function makeInterval(lo, hi, unit) {
  const box = { lo, hi };
  Object.defineProperty(box, INTERVAL_TAG, { value: true, enumerable: false });
  if (unit !== undefined) {
    Object.defineProperty(box, UNIT_TAG, { value: unit, enumerable: false });
  }
  const frozen = Object.freeze(box);
  authenticIntervals.add(frozen);
  return frozen;
}

function requireInterval(value, label) {
  if (typeof value !== "object" || value === null || !authenticIntervals.has(value)) {
    throw invalid(`${label} must come from bap.numeric`);
  }
  return value;
}

function requireUnitValue(value, label) {
  requireInterval(value, label);
  if (typeof value[UNIT_TAG] !== "string") {
    throw invalid(`${label} must carry a unit`);
  }
  return value;
}

function intervalPair(op, left, right) {
  const pair = Deno.core.ops[op](left.lo, left.hi, right.lo, right.hi);
  return makeInterval(pair[0], pair[1], left[UNIT_TAG]);
}

function batchIntervals(list, label) {
  return list.map((item) => {
    requireInterval(item, label);
    return [item.lo, item.hi];
  });
}

function checkBatchShape(left, right, label) {
  if (!Array.isArray(left) || !Array.isArray(right)) {
    throw invalid(`${label} needs two arrays`);
  }
  if (left.length !== right.length) {
    throw invalid(`${label} inputs must have equal lengths`);
  }
  if (left.length > MAX_BATCH_ELEMENTS) {
    throw invalid(`${label} exceeds ${MAX_BATCH_ELEMENTS} elements`);
  }
}

const numeric = {};

numeric.interval = (lo, hi) => {
  const pair = Deno.core.ops.op_numeric_interval_new(
    toNumber(lo, "lo"),
    toNumber(hi, "hi"),
  );
  return makeInterval(pair[0], pair[1]);
};

numeric.unit = (name, value) => {
  const label = toText(name, "unit name");
  if (label.length === 0) throw invalid("unit name must be non-empty");
  const bound = toNumber(value, "value");
  return makeInterval(bound, bound, label);
};

for (const [name, op] of [
  ["intervalAdd", "op_numeric_interval_add"],
  ["intervalSub", "op_numeric_interval_sub"],
  ["intervalMul", "op_numeric_interval_mul"],
  ["intervalDiv", "op_numeric_interval_div"],
]) {
  numeric[name] = (left, right) =>
    intervalPair(op, requireInterval(left, name), requireInterval(right, name));
}

for (const [name, op] of [
  ["intervalAddBatch", "op_numeric_interval_add_batch"],
  ["intervalSubBatch", "op_numeric_interval_sub_batch"],
  ["intervalMulBatch", "op_numeric_interval_mul_batch"],
  ["intervalDivBatch", "op_numeric_interval_div_batch"],
]) {
  numeric[name] = (left, right) => {
    checkBatchShape(left, right, name);
    const pairs = Deno.core.ops[op](
      batchIntervals(left, name),
      batchIntervals(right, name),
    );
    return Object.freeze(pairs.map(([lo, hi]) => makeInterval(lo, hi)));
  };
}

numeric.unitAdd = (left, right) => {
  requireUnitValue(left, "unitAdd");
  requireUnitValue(right, "unitAdd");
  if (left[UNIT_TAG] !== right[UNIT_TAG]) {
    throw invalid("unitAdd operands must share one unit");
  }
  return intervalPair("op_numeric_interval_add", left, right);
};

numeric.unitSub = (left, right) => {
  requireUnitValue(left, "unitSub");
  requireUnitValue(right, "unitSub");
  if (left[UNIT_TAG] !== right[UNIT_TAG]) {
    throw invalid("unitSub operands must share one unit");
  }
  return intervalPair("op_numeric_interval_sub", left, right);
};

numeric.unitMul = (left, right) => {
  requireUnitValue(left, "unitMul");
  requireUnitValue(right, "unitMul");
  if (left[UNIT_TAG] !== DIMENSIONLESS || right[UNIT_TAG] !== DIMENSIONLESS) {
    throw invalid("unitMul operands must be dimensionless");
  }
  return intervalPair("op_numeric_interval_mul", left, right);
};

numeric.unitDiv = (left, right) => {
  requireUnitValue(left, "unitDiv");
  requireUnitValue(right, "unitDiv");
  if (left[UNIT_TAG] !== DIMENSIONLESS || right[UNIT_TAG] !== DIMENSIONLESS) {
    throw invalid("unitDiv operands must be dimensionless");
  }
  return intervalPair("op_numeric_interval_div", left, right);
};

for (const [name, opcode] of [
  ["i32Add", OP_ADD],
  ["i32Sub", OP_SUB],
  ["i32Mul", OP_MUL],
  ["i32Div", OP_DIV],
]) {
  numeric[name] = (lhs, rhs) =>
    Deno.core.ops.op_numeric_i32_binop(
      opcode,
      toInteger(lhs, name),
      toInteger(rhs, name),
    );
}

numeric.i32Neg = (value) =>
  Deno.core.ops.op_numeric_i32_neg(toInteger(value, "i32Neg"));

for (const [name, opcode] of [
  ["i64Add", OP_ADD],
  ["i64Sub", OP_SUB],
  ["i64Mul", OP_MUL],
  ["i64Div", OP_DIV],
]) {
  numeric[name] = (lhs, rhs) =>
    Deno.core.ops.op_numeric_i64_binop(opcode, toBig(lhs, name), toBig(rhs, name));
}

numeric.i64Neg = (value) =>
  Deno.core.ops.op_numeric_i64_neg(toBig(value, "i64Neg"));

numeric.i64AddBatch = (left, right) => {
  checkBatchShape(left, right, "i64AddBatch");
  const packedLeft = left.map((value) => toBig(value, "i64AddBatch"));
  const packedRight = right.map((value) => toBig(value, "i64AddBatch"));
  return Object.freeze(
    Deno.core.ops.op_numeric_i64_add_batch(packedLeft, packedRight),
  );
};

numeric.fixed = (text) =>
  Deno.core.ops.op_numeric_fixed_parse(toText(text, "fixed"));

for (const [name, opcode] of [
  ["fixedAdd", OP_ADD],
  ["fixedSub", OP_SUB],
  ["fixedMul", OP_MUL],
  ["fixedDiv", OP_DIV],
]) {
  numeric[name] = (lhs, rhs) =>
    Deno.core.ops.op_numeric_fixed_binop(
      opcode,
      toText(lhs, name),
      toText(rhs, name),
    );
}

globalThis.__bapNumeric = Object.freeze(numeric);
