// node:process - env, argv approximation, cwd, exit, platform facts.
__bapNodeModules.process = (() => {
  if (!globalThis.process) {
    const env = {};
    for (const [key, value] of Deno.core.ops.op_process_env()) {
      env[key] = value;
    }
    globalThis.process = {
      env,
      // Node's argv is [execPath, scriptPath, ...args]; Bap knows neither at
      // bootstrap time, so the script slot stays empty by design.
      argv: ["bap", "", ...Deno.core.ops.op_bap_args()],
      cwd: () => Deno.core.ops.op_process_cwd(),
      exit: (code = 0) => Deno.core.ops.op_process_exit(code),
      nextTick: (fn, ...args) => queueMicrotask(() => fn(...args)),
      stdout: {
        // Exact-byte writes; console ops always append a newline.
        write: (text) => {
          Deno.core.print(String(text));
          return true;
        },
      },
      version: "v22.0.0-bap",
      versions: { node: "22.0.0", bap: "0.2.0" },
      ...Deno.core.ops.op_process_info(),
    };
  }
  return globalThis.process;
})();
