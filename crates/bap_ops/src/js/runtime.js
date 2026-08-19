function encodeUtf8(str) {
  const bytes = [];
  for (let i = 0; i < str.length; i++) {
    const cp = str.codePointAt(i);
    if (cp > 0xffff) i++;
    if (cp < 0x80) {
      bytes.push(cp);
    } else if (cp < 0x800) {
      bytes.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
    } else if (cp < 0x10000) {
      bytes.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
    } else {
      bytes.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
    }
  }
  return Uint8Array.from(bytes);
}

function decodeUtf8(bytes) {
  const points = [];
  let i = 0;
  while (i < bytes.length) {
    const b = bytes[i];
    let cp;
    if (b < 0x80) {
      cp = b;
      i += 1;
    } else if (b < 0xe0) {
      cp = ((b & 0x1f) << 6) | (bytes[i + 1] & 0x3f);
      i += 2;
    } else if (b < 0xf0) {
      cp = ((b & 0x0f) << 12) | ((bytes[i + 1] & 0x3f) << 6) | (bytes[i + 2] & 0x3f);
      i += 3;
    } else {
      cp = ((b & 0x07) << 18) | ((bytes[i + 1] & 0x3f) << 12) | ((bytes[i + 2] & 0x3f) << 6) | (bytes[i + 3] & 0x3f);
      i += 4;
    }
    points.push(cp);
  }
  return String.fromCodePoint(...points);
}

function setTimeout(fn, ms) {
  const id = Deno.core.ops.op_timer_create();
  Deno.core.ops.op_timer_set_timeout(id, ms).then((armed) => {
    if (armed) fn();
  });
  return id;
}

function clearTimeout(id) {
  Deno.core.ops.op_timer_clear(id);
}

const bap = {};

bap.readFile = (path) => Deno.core.ops.op_fs_read_file(String(path));
bap.writeFile = (path, data) =>
  Deno.core.ops.op_fs_write_file(
    String(path),
    typeof data === "string" ? encodeUtf8(data) : data,
  );
bap.readDir = (path) => Deno.core.ops.op_fs_read_dir(String(path));
bap.stat = (path) => Deno.core.ops.op_fs_stat(String(path));

globalThis.bap = Object.freeze(bap);
globalThis.setTimeout = setTimeout;
globalThis.clearTimeout = clearTimeout;