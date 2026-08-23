/**
 * Ambient types for the bap numeric surface.
 * The hidden brands exist at runtime as non-enumerable properties assigned by
 * `js/numeric.js`; forged lookalikes are rejected there.
 */
interface BapInterval {
  readonly lo: number;
  readonly hi: number;
  readonly __bapInterval: true;
}

interface BapUnit {
  readonly lo: number;
  readonly hi: number;
  readonly __bapInterval: true;
  readonly __bapUnit: string;
}

declare namespace BapNumeric {
  type Unit = BapUnit;

  function interval(lo: number, hi: number): BapInterval;
  function intervalAdd(left: BapInterval, right: BapInterval): BapInterval;
  function intervalSub(left: BapInterval, right: BapInterval): BapInterval;
  function intervalMul(left: BapInterval, right: BapInterval): BapInterval;
  function intervalDiv(left: BapInterval, right: BapInterval): BapInterval;
  function intervalAddBatch(
    left: readonly BapInterval[],
    right: readonly BapInterval[],
  ): readonly BapInterval[];
  function intervalSubBatch(
    left: readonly BapInterval[],
    right: readonly BapInterval[],
  ): readonly BapInterval[];
  function intervalMulBatch(
    left: readonly BapInterval[],
    right: readonly BapInterval[],
  ): readonly BapInterval[];
  function intervalDivBatch(
    left: readonly BapInterval[],
    right: readonly BapInterval[],
  ): readonly BapInterval[];

  function unit(name: string, value: number): Unit;
  function unitAdd(left: Unit, right: Unit): Unit;
  function unitSub(left: Unit, right: Unit): Unit;
  /** Both operands must be dimensionless units. */
  function unitMul(left: Unit, right: Unit): Unit;
  /** Both operands must be dimensionless units. */
  function unitDiv(left: Unit, right: Unit): Unit;

  function i32Add(lhs: number, rhs: number): number;
  function i32Sub(lhs: number, rhs: number): number;
  function i32Mul(lhs: number, rhs: number): number;
  function i32Div(lhs: number, rhs: number): number;
  function i32Neg(value: number): number;

  function i64Add(lhs: bigint, rhs: bigint): bigint;
  function i64Sub(lhs: bigint, rhs: bigint): bigint;
  function i64Mul(lhs: bigint, rhs: bigint): bigint;
  function i64Div(lhs: bigint, rhs: bigint): bigint;
  function i64Neg(value: bigint): bigint;
  function i64AddBatch(left: readonly bigint[], right: readonly bigint[]): readonly bigint[];

  function fixed(text: string): string;
  function fixedAdd(lhs: string, rhs: string): string;
  function fixedSub(lhs: string, rhs: string): string;
  function fixedMul(lhs: string, rhs: string): string;
  function fixedDiv(lhs: string, rhs: string): string;
}

declare const bap: {
  readonly numeric: typeof BapNumeric;
};
