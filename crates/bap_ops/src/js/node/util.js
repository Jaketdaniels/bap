// node:util - formatting, inspection, and promisify.
__bapNodeModules.util = (() => {
  function format(value) {
    if (typeof value === "string") return value;
    return inspect(value);
  }

  function formatWith(pattern, args) {
    let index = 0;
    let out = String(pattern).replace(/%[sdjifoO%]/g, (token) => {
      if (token === "%%") return "%";
      if (index >= args.length) return token;
      const arg = args[index++];
      switch (token) {
        case "%s": return String(arg);
        case "%d":
        case "%f": return Number(arg).toString();
        case "%j": return JSON.stringify(arg);
        case "%i": return Math.trunc(Number(arg)).toString();
        case "%o":
        case "%O": return inspect(arg);
        default: return token;
      }
    });
    for (; index < args.length; index++) {
      out += " " + format(args[index]);
    }
    return out;
  }

  function inspect(value) {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (typeof value === "string") return `'${value}'`;
    if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
      return value.toString();
    }
    if (typeof value === "function") return `[Function: ${value.name || "anonymous"}]`;
    if (Array.isArray(value)) {
      return `[ ${value.map((item) => inspect(item)).join(", ")} ]`;
    }
    if (value instanceof Error) return value.stack || value.toString();
    try {
      const entries = Object.entries(value)
        .map(([key, item]) => `${key}: ${inspect(item)}`)
        .join(", ");
      const name = value.constructor?.name;
      return `${name && name !== "Object" ? `${name} ` : ""}{ ${entries} }`;
    } catch {
      return String(value);
    }
  }

  function promisify(fn) {
    return function promisified(...args) {
      return new Promise((resolve, reject) => {
        fn.call(this, ...args, (error, result) => {
          if (error) reject(error);
          else resolve(result);
        });
      });
    };
  }

  function deprecate(fn, message) {
    let warned = false;
    return function deprecated(...args) {
      if (!warned) {
        warned = true;
        console.warn(`(node:bap) DeprecationWarning: ${message}`);
      }
      return fn.apply(this, args);
    };
  }

  return Object.freeze({
    format: (...args) => (args.length <= 1 ? format(args[0]) : formatWith(args[0], args.slice(1))),
    inspect,
    promisify,
    deprecate,
    isArray: Array.isArray,
    inherits: (ctor, superCtor) => Object.setPrototypeOf(ctor.prototype, superCtor.prototype),
  });
})();
