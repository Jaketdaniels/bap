// node:fs - sync surface over sync ops, promise surface over async ops.
__bapNodeModules.fs = (() => {
  const ops = Deno.core.ops;

  function toBuffer(data) {
    if (typeof data === "string") return Buffer.from(data, "utf8");
    return Buffer.from(data);
  }

  function decode(bytes, encoding) {
    if (!encoding) return Buffer.from(bytes);
    const text = decodeUtf8(bytes);
    if (String(encoding).replace(/-/g, "").toLowerCase() === "utf8") return text;
    throw new Error(`bap: fs encoding '${encoding}' is not supported (utf8 only)`);
  }

  function readFileSync(path, options) {
    const bytes = ops.op_fs_read_file_sync(String(path));
    const encoding = typeof options === "string" ? options : options?.encoding;
    return decode(bytes, encoding);
  }

  function writeFileSync(path, data) {
    ops.op_fs_write_file_sync(String(path), toBuffer(data));
  }

  function existsSync(path) {
    return ops.op_fs_exists_sync(String(path));
  }

  function statSync(path) {
    const stat = ops.op_fs_stat_sync(String(path));
    return {
      size: stat.size,
      mtimeMs: stat.modifiedMs,
      isDirectory: () => stat.isDir,
      isFile: () => stat.isFile,
    };
  }

  function readdirSync(path) {
    return ops.op_fs_read_dir_sync(String(path)).map((entry) => entry.name);
  }

  function mkdirSync(path) {
    Deno.core.ops.op_fs_make_dir_sync(String(path));
  }

  const promises = {
    readFile: async (path, options) => {
      const bytes = await ops.op_fs_read_file(String(path));
      return decode(bytes, typeof options === "string" ? options : options?.encoding);
    },
    writeFile: async (path, data) => {
      await ops.op_fs_write_file(String(path), toBuffer(data));
    },
    stat: async (path) => statSync(path),
    readdir: async (path) => readdirSync(path),
  };

  return Object.freeze({
    readFileSync,
    writeFileSync,
    existsSync,
    statSync,
    readdirSync,
    mkdirSync,
    promises,
    constants: Object.freeze({ F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1 }),
  });
})();
