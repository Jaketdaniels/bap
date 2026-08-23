// node:path - POSIX path helpers over plain strings.
__bapNodeModules.path = (() => {
  function normalize(path) {
    const absolute = path.startsWith("/");
    const out = [];
    for (const segment of String(path).split("/")) {
      if (segment === "" || segment === ".") continue;
      if (segment === "..") {
        if (out.length > 0 && out[out.length - 1] !== "..") out.pop();
        else if (!absolute) out.push("..");
        continue;
      }
      out.push(segment);
    }
    const joined = out.join("/");
    return (absolute ? "/" : "") + (joined || (absolute ? "" : "."));
  }

  function join(...parts) {
    return normalize(parts.filter((part) => part !== "").join("/"));
  }

  function resolve(...parts) {
    let resolved = process.cwd();
    for (const part of parts) {
      if (part.startsWith("/")) resolved = part;
      else resolved = resolved + "/" + part;
    }
    return normalize(resolved);
  }

  function isAbsolute(path) {
    return String(path).startsWith("/");
  }

  function dirname(path) {
    const text = String(path);
    const index = text.lastIndexOf("/");
    if (index < 0) return ".";
    if (index === 0) return "/";
    return text.slice(0, index).replace(/\/+$/, "") || "/";
  }

  function basename(path, suffix) {
    const text = String(path).replace(/\/+$/, "");
    const name = text.slice(text.lastIndexOf("/") + 1) || "/";
    if (suffix && name.endsWith(suffix)) return name.slice(0, -suffix.length);
    return name;
  }

  function extname(path) {
    const name = basename(String(path));
    const dot = name.lastIndexOf(".");
    if (dot <= 0) return "";
    return name.slice(dot);
  }

  function relative(from, to) {
    const fromParts = resolve(from).split("/").filter(Boolean);
    const toParts = resolve(to).split("/").filter(Boolean);
    let shared = 0;
    while (
      shared < fromParts.length
      && shared < toParts.length
      && fromParts[shared] === toParts[shared]
    ) shared++;
    const ups = fromParts.length - shared;
    const downs = toParts.slice(shared);
    return [...Array(ups).fill(".."), ...downs].join("/") || ".";
  }

  return Object.freeze({
    normalize,
    join,
    resolve,
    isAbsolute,
    dirname,
    basename,
    extname,
    relative,
    sep: "/",
    delimiter: ":",
  });
})();
