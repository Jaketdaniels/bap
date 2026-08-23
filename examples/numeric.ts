// Demonstrates the audited numeric surface: intervals, checked ints, and fixed point.
const span = bap.numeric.interval(0.95, 1.05);
const gain = bap.numeric.interval(2, 4);
const product = bap.numeric.intervalMul(span, gain);
console.log(`interval [${span.lo}, ${span.hi}] x [${gain.lo}, ${gain.hi}] = [${product.lo}, ${product.hi}]`);

const batch = bap.numeric.intervalAddBatch(
  [bap.numeric.interval(1, 1), bap.numeric.interval(2, 2)],
  [bap.numeric.interval(3, 3), bap.numeric.interval(4, 4)],
);
console.log(`batch sums -> [${batch.map((item) => `${item.lo}..${item.hi}`).join(", ")}]`);

const price: string = bap.numeric.fixed("12.3456");
const quantity: string = bap.numeric.fixedAdd("3", "0.5");
const total: string = bap.numeric.fixedMul(price, quantity);
console.log(`fixed total: ${total}`);

console.log(`checked i64: ${bap.numeric.i64Mul(3037000499n, 3037000499n)}`);
console.log(
  `unit total: ${bap.numeric.unitAdd(bap.numeric.unit("metre", 7), bap.numeric.unit("metre", 8)).lo}`,
);

try {
  bap.numeric.i64Add(9223372036854775807n, 1n);
} catch (error) {
  console.log(`overflow surfaced as ${(error as Error).message}`);
}
