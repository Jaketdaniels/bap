/// <reference path="../../src/js/numeric.d.ts" />

// Compile-time checks for the numeric ambient types; verified with tsc --noEmit.

const interval: BapInterval = bap.numeric.interval(0, 1);
const sum: BapInterval = bap.numeric.intervalAdd(
  interval,
  bap.numeric.interval(1, 2),
);
const lo: number = sum.lo;

const metre: BapNumeric.Unit = bap.numeric.unit("metre", 7);
const metres: BapNumeric.Unit = bap.numeric.unitAdd(metre, metre);

const exact: bigint = bap.numeric.i64Add(1n, 2n);
const decimal: string = bap.numeric.fixedAdd("1.5", "2.25");
const batch: readonly BapInterval[] = bap.numeric.intervalAddBatch([sum], [sum]);

// @ts-expect-error forged intervals are rejected at runtime and ill-typed here
bap.numeric.intervalAdd({ lo: 0, hi: 1 }, sum);

// @ts-expect-error unit addition requires unit values
bap.numeric.unitAdd(sum, sum);

// @ts-expect-error i64 helpers take bigints
bap.numeric.i64Add(1, 2);

export { batch, decimal, exact, lo, metres };
