// CommonJS support: synchronous resolution, wrapping, and caching.
// ESM files import CJS modules through globalThis.__bapCjs.load(absPath).

const __bapNodeModules = {};

const __bapCjsModuleCache = new Map();
const __bapCjsBuiltinNames = new Set(Object.keys(__bapNodeModules));

function __bapCjsDirname(path) {
  const index = path.lastIndexOf("/");
  if (index < 0) return ".";
  if (index === 0) return "/";
  return path.slice(0, index);
}

function __bapCjsIsBare(request) {
  return !request.startsWith("./") && !request.startsWith("../")
    && !request.startsWith("/") && request !== "." && request !== "..";
}

// Collapses "//", "/./", and lexical ".." segments on absolute paths so
// nested requires cannot grow the dirname on every hop.
function __bapCjsNormalize(absolute) {
  const segments = absolute.split("/");
  const out = [];
  for (const segment of segments.slice(1)) {
    if (segment === "" || segment === ".") continue;
    if (segment === ".." && out.length > 0 && out[out.length - 1] !== "..") {
      out.pop();
      continue;
    }
    out.push(segment);
  }
  return `/${out.join("/")}`;
}

function __bapCjsExists(path) {
  return Deno.core.ops.op_fs_exists_sync(path);
}

function __bapCjsReadBytes(path) {
  return Deno.core.ops.op_fs_read_file_sync(path);
}

function __bapCjsResolveFile(candidate) {
  for (const suffix of ["", ".js", ".cjs", ".json"]) {
    const target = candidate + suffix;
    if (__bapCjsExists(target)) return target;
  }
  return null;
}

function __bapCjsResolveDirectory(directory) {
  const pkg = directory + "/package.json";
  if (__bapCjsExists(pkg)) {
    try {
      const manifest = JSON.parse(decodeUtf8(__bapCjsReadBytes(pkg)));
      if (typeof manifest.main === "string" && manifest.main.length > 0) {
        const base = manifest.main.startsWith("/")
          ? manifest.main
          : directory + "/" + manifest.main;
        const file = __bapCjsResolveFile(base)
          ?? (manifest.main.endsWith(".js") || manifest.main.endsWith(".cjs")
            ? null
            : __bapCjsResolveDirectory(base));
        if (file) return file;
      }
    } catch {
      // A malformed package.json falls through to index probing.
    }
  }
  for (const index of ["/index.js", "/index.cjs"]) {
    if (__bapCjsExists(directory + index)) return directory + index;
  }
  return null;
}

function __bapCjsResolve(request, parentDir) {
  let name = request;
  if (name.startsWith("node:")) name = name.slice(5);
  if (__bapCjsIsBare(name)) {
    if (__bapNodeModules[name] !== undefined) return { builtin: name };
    const aliases = globalThis.__bapAliases;
    if (aliases && typeof aliases[name] === "string") {
      const mapped = aliases[name].startsWith("/")
        ? aliases[name]
        : parentDir + "/" + aliases[name];
      const resolved = __bapCjsResolveFile(mapped) ?? __bapCjsResolveDirectory(mapped);
      if (resolved) return resolved;
    }
    throw new Error(
      `bap: cannot find module '${request}' (npm packages are not supported on Bap yet)`,
    );
  }
  const base = __bapCjsNormalize(
    name.startsWith("/") ? name : `${parentDir}/${name}`,
  );
  return __bapCjsResolveFile(base) ?? __bapCjsResolveDirectory(base)
    ?? (() => {
      throw new Error(`bap: cannot find module '${request}' from '${parentDir}'`);
    })();
}

function __bapCjsEvaluate(resolvedPath, source) {
  const dirname = __bapCjsDirname(resolvedPath);
  const module = { exports: {}, filename: resolvedPath };
  // Cache before evaluation so require cycles see partial exports.
  __bapCjsModuleCache.set(resolvedPath, module);
  const initialize = new Function(
    "module",
    "exports",
    "require",
    "__filename",
    "__dirname",
    source,
  );
  initialize.call(
    module.exports,
    module,
    module.exports,
    __bapCjsMakeRequire(dirname),
    resolvedPath,
    dirname,
  );
  return module;
}

function __bapCjsLoad(resolvedPath) {
  const cached = __bapCjsModuleCache.get(resolvedPath);
  if (cached) return cached;
  const bytes = __bapCjsReadBytes(resolvedPath);
  if (resolvedPath.endsWith(".json")) {
    const module = { exports: JSON.parse(decodeUtf8(bytes)), filename: resolvedPath };
    __bapCjsModuleCache.set(resolvedPath, module);
    return module;
  }
  return __bapCjsEvaluate(resolvedPath, decodeUtf8(bytes));
}

function __bapCjsMakeRequire(parentDir) {
  const require = (request) => {
    const resolved = __bapCjsResolve(String(request), parentDir);
    if (typeof resolved === "object") {
      return __bapNodeModules[resolved.builtin];
    }
    return __bapCjsLoad(resolved).exports;
  };
  require.resolve = (request) => {
    const resolved = __bapCjsResolve(String(request), parentDir);
    if (typeof resolved === "object") return `node:${resolved.builtin}`;
    return resolved;
  };
  require.cache = __bapCjsModuleCache;
  return require;
}

globalThis.__bapCjs = Object.freeze({
  load: __bapCjsLoad,
  resolve: (request, parentDir) => {
    const resolved = __bapCjsResolve(String(request), String(parentDir));
    return typeof resolved === "object" ? `node:${resolved.builtin}` : resolved;
  },
});
