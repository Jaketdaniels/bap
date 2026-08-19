function formatArg(value) {
  if (value === null) return "null";
  if (value === undefined) return "undefined";
  if (typeof value === "string") return value;
  if (typeof value === "bigint") return value.toString() + "n";
  if (typeof value === "function") return value.toString();
  if (value instanceof Error) return value.stack || value.toString();
  try {
    return JSON.stringify(value, (_, val) =>
      typeof val === "bigint" ? val.toString() + "n" : val,
    );
  } catch {
    return String(value);
  }
}

function formatArgs(args) {
  return args.map(formatArg).join(" ");
}

globalThis.console = {
  log: (...args) => Deno.core.ops.op_console_log(formatArgs(args) + "\n"),
  error: (...args) => Deno.core.ops.op_console_error(formatArgs(args) + "\n"),
  warn: (...args) => Deno.core.ops.op_console_warn(formatArgs(args) + "\n"),
};

globalThis.Bap = Object.freeze({
  args: Deno.core.ops.op_bap_args(),
});