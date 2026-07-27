import { describe, it, expect } from 'vitest';
import { inflateSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import {
  assertOpaque,
  decodeCanonicalPng,
  encodeCanonicalPng,
  type RawFramebuffer,
} from '../e2e/canonicalImage';

// The encoder that decides what a canonical view's committed bytes are.
//
// The acceptance criterion for the map views is that two clean loads produce byte-identical PNGs, so
// this encoder is load-bearing for the gate rather than a convenience: if it were free to vary, the
// gate would be testing the encoder's mood. What is checked here is that it does not — that the same
// pixels give the same bytes, that no timestamp or other varying chunk is emitted, that the image is
// stored the right way up, and that the one assumption it makes (an opaque frame) is enforced rather
// than assumed.

/** A deterministic, structured test image. Gradients so filtering has something to predict. */
function fixture(width: number, height: number): RawFramebuffer {
  const rgba = Buffer.alloc(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      rgba[i] = (x * 7 + y * 3) & 0xff;
      rgba[i + 1] = (x * 2 + y * 11) & 0xff;
      rgba[i + 2] = (x * x + y) & 0xff;
      rgba[i + 3] = 0xff;
    }
  }
  return { width, height, rgba };
}

const sha = (b: Buffer): string => createHash('sha256').update(b).digest('hex');

describe('canonical PNG encoder', () => {
  it('round-trips the pixels exactly', () => {
    const fb = fixture(37, 19);
    const back = decodeCanonicalPng(encodeCanonicalPng(fb));
    expect(back.width).toBe(fb.width);
    expect(back.height).toBe(fb.height);
    expect(back.rgba.equals(fb.rgba)).toBe(true);
  });

  it('produces identical bytes for identical pixels', () => {
    // The property the whole gate rests on. If this ever fails, "the PNG differs" would stop meaning
    // "the pixels differ" and the byte-identity criterion would be measuring the encoder.
    const a = encodeCanonicalPng(fixture(64, 40));
    const b = encodeCanonicalPng(fixture(64, 40));
    expect(sha(a)).toBe(sha(b));
  });

  it('produces different bytes for a single changed channel', () => {
    // The other half: a gate that cannot fail is not a gate. One level of one channel of one pixel.
    const a = fixture(64, 40);
    const b = fixture(64, 40);
    b.rgba[(20 * 64 + 31) * 4 + 1] ^= 1;
    expect(sha(encodeCanonicalPng(a))).not.toBe(sha(encodeCanonicalPng(b)));
  });

  it('emits no chunk that could vary between two encodes of one image', () => {
    // `tIME` is the specific trap: a conforming encoder may add it, and one timestamp chunk would
    // make every capture differ from every other capture while every pixel matched.
    const png = encodeCanonicalPng(fixture(8, 8));
    const types: string[] = [];
    let off = 8;
    while (off + 8 <= png.length) {
      const len = png.readUInt32BE(off);
      types.push(png.toString('ascii', off + 4, off + 8));
      off += 12 + len;
    }
    expect(types).toEqual(['IHDR', 'IDAT', 'IEND']);
  });

  it('stores the framebuffer the right way up', () => {
    // Independent of `decodeCanonicalPng`, which would pass a round-trip even if the flip were
    // missing on both sides. A 1x2 image: input row 0 is the BOTTOM of the picture, so PNG's first
    // scanline must be input row 1.
    //
    // Paeth on the first row has no left pixel and no row above, so `paeth(0,0,0) = 0` and the
    // filtered bytes are the raw bytes — which is what makes this readable without unfiltering.
    const fb: RawFramebuffer = {
      width: 1,
      height: 2,
      rgba: Buffer.from([10, 20, 30, 255, 200, 210, 220, 255]),
    };
    const png = encodeCanonicalPng(fb);
    let idat = Buffer.alloc(0);
    let off = 8;
    while (off + 8 <= png.length) {
      const len = png.readUInt32BE(off);
      if (png.toString('ascii', off + 4, off + 8) === 'IDAT') idat = png.subarray(off + 8, off + 8 + len);
      off += 12 + len;
    }
    const raw = inflateSync(idat);
    expect(raw[0]).toBe(4); // Paeth
    expect([...raw.subarray(1, 5)]).toEqual([200, 210, 220, 255]);
  });

  it('refuses a frame that is not fully opaque, and says where', () => {
    const fb = fixture(4, 3);
    fb.rgba[(1 * 4 + 2) * 4 + 3] = 0x80;
    expect(() => assertOpaque(fb)).toThrow(/alpha 128 at pixel \(2, 1 from the bottom\)/);
    expect(() => encodeCanonicalPng(fb)).toThrow(/alpha: false/);
  });

  it('refuses a buffer whose length disagrees with its declared size', () => {
    const fb = fixture(10, 10);
    expect(() => encodeCanonicalPng({ ...fb, width: 11 })).toThrow(/needs 440 bytes of RGBA, got 400/);
    expect(() => encodeCanonicalPng({ ...fb, height: 0 })).toThrow(/implausible framebuffer size/);
  });
});
