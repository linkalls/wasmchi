export class Bytes {
  private buf: number[] = [];

  u8(n: number): this {
    this.buf.push(n & 0xff);
    return this;
  }

  u32LE(n: number): this {
    this.u8(n);
    this.u8(n >>> 8);
    this.u8(n >>> 16);
    this.u8(n >>> 24);
    return this;
  }

  // unsigned LEB128
  uleb(n: number): this {
    let v = n >>> 0;
    while (true) {
      const byte = v & 0x7f;
      v >>>= 7;
      if (v === 0) {
        this.u8(byte);
        return this;
      }
      this.u8(byte | 0x80);
    }
  }

  // signed LEB128 for i32 (minimal-ish)
  sleb32(n: number): this {
    let v = n | 0;
    let more = true;
    while (more) {
      let byte = v & 0x7f;
      v >>= 7;
      const signBit = (byte & 0x40) !== 0;
      more = !((v === 0 && !signBit) || (v === -1 && signBit));
      if (more) byte |= 0x80;
      this.u8(byte);
    }
    return this;
  }

  bytes(arr: ArrayLike<number>): this {
    for (let i = 0; i < arr.length; i++) this.u8(arr[i]!);
    return this;
  }

  vec(chunks: Uint8Array[]): this {
    const total = chunks.reduce((a, b) => a + b.length, 0);
    const out = new Uint8Array(total);
    let o = 0;
    for (const c of chunks) {
      out.set(c, o);
      o += c.length;
    }
    this.bytes(out);
    return this;
  }

  toU8(): Uint8Array {
    return new Uint8Array(this.buf);
  }
}

export function encodeString(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}
