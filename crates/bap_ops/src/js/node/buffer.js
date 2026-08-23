// node:buffer - Buffer over Uint8Array with utf8/hex/base64 support.
__bapNodeModules.buffer = (() => {
  const HEX = "0123456789abcdef";
  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

  const hexEncode = (bytes) => {
    let out = "";
    for (const byte of bytes) out += HEX[byte >> 4] + HEX[byte & 0xf];
    return out;
  };

  const hexDecode = (text) => {
    const clean = text.replace(/[^0-9a-fA-F]/g, "");
    const bytes = new Uint8Array(clean.length >> 1);
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
    }
    return bytes;
  };

  const base64Encode = (bytes) => {
    let out = "";
    for (let i = 0; i < bytes.length; i += 3) {
      const b1 = bytes[i];
      const b2 = bytes[i + 1];
      const b3 = bytes[i + 2];
      out += B64[b1 >> 2];
      out += B64[((b1 & 3) << 4) | ((b2 ?? 0) >> 4)];
      out += b2 === undefined ? "=" : B64[((b2 & 15) << 2) | ((b3 ?? 0) >> 6)];
      out += b3 === undefined ? "=" : B64[b3 & 63];
    }
    return out;
  };

  const base64Decode = (text) => {
    const clean = text.replace(/[^A-Za-z0-9+/=]/g, "").replace(/=+$/, "");
    const byteLength = Math.floor((clean.length * 3) / 4);
    const bytes = new Uint8Array(byteLength);
    let position = 0;
    for (let i = 0; i < clean.length; i += 4) {
      const chunk =
        (B64.indexOf(clean[i]) << 18)
        | ((B64.indexOf(clean[i + 1]) || 0) << 12)
        | ((B64.indexOf(clean[i + 2]) || 0) << 6)
        | (B64.indexOf(clean[i + 3]) || 0);
      if (position < byteLength) bytes[position++] = (chunk >> 16) & 0xff;
      if (position < byteLength && clean[i + 2] !== undefined) bytes[position++] = (chunk >> 8) & 0xff;
      if (position < byteLength && clean[i + 3] !== undefined) bytes[position++] = chunk & 0xff;
    }
    return bytes.subarray(0, position);
  };

  class Buffer extends Uint8Array {
    toString(encoding = "utf8", start = 0, end = this.length) {
      const sliceBytes = this.subarray(start, end);
      switch (String(encoding).toLowerCase()) {
        case "utf8":
        case "utf-8":
          return decodeUtf8(sliceBytes);
        case "hex":
          return hexEncode(sliceBytes);
        case "base64":
          return base64Encode(sliceBytes);
        case "latin1":
        case "binary":
          return String.fromCharCode(...sliceBytes);
        default:
          throw new TypeError(`bap: unsupported Buffer encoding '${encoding}'`);
      }
    }

    equals(other) {
      if (!Buffer.isBuffer(other) || other.length !== this.length) return false;
      for (let i = 0; i < this.length; i++) {
        if (this[i] !== other[i]) return false;
      }
      return true;
    }

    indexOf(value, byteOffset = 0) {
      if (typeof value === "string") return this.indexOf(Buffer.from(value, "utf8"), byteOffset);
      if (Buffer.isBuffer(value)) {
        outer: for (let i = Math.max(0, byteOffset); i <= this.length - value.length; i++) {
          for (let j = 0; j < value.length; j++) {
            if (this[i + j] !== value[j]) continue outer;
          }
          return i;
        }
        return -1;
      }
      for (let i = byteOffset; i < this.length; i++) {
        if (this[i] === value) return i;
      }
      return -1;
    }

    slice(start = 0, end = this.length) {
      const copy = new Buffer(Math.max(0, end - start));
      copy.set(this.subarray(start, end));
      return copy;
    }

    toJSON() {
      return { type: "Buffer", data: Array.from(this) };
    }
  }

  function encodingBytes(text, encoding) {
    switch (String(encoding ?? "utf8").toLowerCase()) {
      case "utf8":
      case "utf-8":
        return encodeUtf8(String(text));
      case "hex":
        return hexDecode(String(text));
      case "base64":
        return base64Decode(String(text));
      case "latin1":
      case "binary": {
        const bytes = new Uint8Array(text.length);
        for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i) & 0xff;
        return bytes;
      }
      default:
        throw new TypeError(`bap: unsupported Buffer encoding '${encoding}'`);
    }
  }

  Buffer.from = (value, encoding) => {
    if (typeof value === "string") {
      const bytes = encodingBytes(value, encoding);
      const buffer = new Buffer(bytes.length);
      buffer.set(bytes);
      return buffer;
    }
    if (ArrayBuffer.isView(value)) {
      const view = Uint8Array.from(value);
      const buffer = new Buffer(view.length);
      buffer.set(view);
      return buffer;
    }
    if (Array.isArray(value) || value?.[Symbol.iterator]) {
      const buffer = new Buffer([...value].length);
      buffer.set([...value]);
      return buffer;
    }
    throw new TypeError("Buffer.from argument must be a string, view, or iterable");
  };

  Buffer.alloc = (size, fill = 0) => {
    const buffer = new Buffer(size);
    if (fill !== 0) {
      const fillBytes = typeof fill === "string"
        ? encodingBytes(fill, "utf8")
        : fill instanceof Uint8Array ? fill : new Uint8Array([Number(fill)]);
      for (let i = 0; i < size; i++) buffer[i] = fillBytes[i % fillBytes.length];
    }
    return buffer;
  };

  Buffer.allocUnsafe = Buffer.alloc;

  Buffer.concat = (list) => {
    const total = list.reduce((sum, item) => sum + item.length, 0);
    const buffer = new Buffer(total);
    let offset = 0;
    for (const item of list) {
      buffer.set(item, offset);
      offset += item.length;
    }
    return buffer;
  };

  Buffer.isBuffer = (value) => value instanceof Buffer;

  Buffer.byteLength = (value, encoding) => {
    if (typeof value !== "string") return value.length;
    return encodingBytes(value, encoding).length;
  };

  globalThis.Buffer = globalThis.Buffer ?? Buffer;
  return Object.freeze({ Buffer: globalThis.Buffer });
})();
