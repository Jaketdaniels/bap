// node:assert - the assertion vocabulary most Node test suites reach for.
__bapNodeModules.assert = (() => {
  function fail(message) {
    throw new AssertionError(message ?? "Failed");
  }

  class AssertionError extends Error {
    constructor(options) {
      const text = typeof options === "string"
        ? options
        : options?.message ?? "Assertion failed";
      super(text);
      this.name = "AssertionError";
      if (typeof options === "object" && options !== null) {
        this.actual = options.actual;
        this.expected = options.expected;
        this.operator = options.operator;
      }
    }
  }

  function ok(value, message) {
    if (!value) fail(message ?? `Expected truthy value, got ${inspectShort(value)}`);
  }

  function equal(actual, expected, message) {
    if (actual != expected) {
      throw new AssertionError({ actual, expected, operator: "==", message });
    }
  }

  function notEqual(actual, expected, message) {
    if (actual == expected) {
      throw new AssertionError({ actual, expected, operator: "!=", message });
    }
  }

  function strictEqual(actual, expected, message) {
    if (actual !== expected) {
      throw new AssertionError({
        actual,
        expected,
        operator: "strictEqual",
        message: message ?? `${inspectShort(actual)} !== ${inspectShort(expected)}`,
      });
    }
  }

  function notStrictEqual(actual, expected, message) {
    if (actual === expected) {
      throw new AssertionError({ actual, expected, operator: "!=", message });
    }
  }

  function deepEqual(actual, expected, message) {
    if (!deepStrictEqualValue(actual, expected)) {
      throw new AssertionError({
        actual,
        expected,
        operator: "deepEqual",
        message: message ?? "deep equality failed",
      });
    }
  }

  const deepStrictEqual = deepEqual;

  function deepStrictEqualValue(actual, expected) {
    if (actual === expected) return true;
    if (typeof actual !== typeof expected) return false;
    if (actual === null || typeof actual !== "object") return false;
    if (Array.isArray(actual) !== Array.isArray(expected)) return false;
    const actualKeys = Object.keys(actual);
    const expectedKeys = Object.keys(expected);
    if (actualKeys.length !== expectedKeys.length) return false;
    return actualKeys.every((key) =>
      deepStrictEqualValue(actual[key], expected[key]),
    );
  }

  function inspectShort(value) {
    try {
      return JSON.stringify(value) ?? String(value);
    } catch {
      return String(value);
    }
  }

  function throws(fn, _expected, message) {
    let threw = false;
    try {
      fn();
    } catch {
      threw = true;
    }
    if (!threw) fail(message ?? "Expected function to throw");
  }

  async function rejects(promiseOrFn, _expected, message) {
    let threw = false;
    try {
      await (typeof promiseOrFn === "function" ? promiseOrFn() : promiseOrFn);
    } catch {
      threw = true;
    }
    if (!threw) fail(message ?? "Expected promise to reject");
  }

  function doesNotThrow(fn, _expected, message) {
    try {
      fn();
    } catch (error) {
      fail(message ?? `Unexpected throw: ${error}`);
    }
  }

  function ifError(value) {
    if (value) fail(`Unexpected error: ${value}`);
  }

  return Object.freeze({
    AssertionError,
    ok,
    fail,
    equal,
    notEqual,
    strictEqual,
    notStrictEqual,
    deepEqual,
    deepStrictEqual,
    throws,
    rejects,
    doesNotThrow,
    ifError,
  });
})();
