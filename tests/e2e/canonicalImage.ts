// The saved half of the canonical-view pipeline: raw framebuffer bytes in, the exact PNG that gets
// committed out.
//
// # Why the harness encodes its own PNG
//
// Acceptance for a canonical view is that two equivalently clean loads produce **byte-identical saved
// PNGs**, and therefore one SHA-256. That is a statement about the file on disk, so every byte
// between the framebuffer and the file has to be a function of the framebuffer and nothing else.
//
// `canvas.screenshot()` is not. It asks Chromium to composite the page region the element occupies
// and encode the result with its own encoder, so the saved bytes depend on the compositor's
// colour-space handling, the encoder's build, and anything stacked over the canvas. None of that is
// wrong for a screenshot; all of it is an unmeasured input to a byte-identity gate. It also captures
// a *page region* rather than the element's pixel buffer, which is how an earlier pass produced eight
// "canonical map views" with the HUD burned into them.
//
// So the harness reads the drawing buffer with `readPixels` and encodes here. The saved file is then
// a pure function of `(width, height, pixels)`, and "the PNG differs" can only mean "the pixels
// differ".
//
// # The canonicalisation, in full
//
// Exactly one transform, and it is information-preserving:
//
//   **Row order.** `readPixels` returns rows bottom-to-top (WebGL's origin is bottom-left); PNG
//   stores them top-to-bottom. The rows are reversed. Nothing is resampled, no channel is touched,
//   RGBA8 goes in and RGBA8 comes out in that order — no premultiply, no colour conversion, no
//   gamma, no quantisation.
//
// There is deliberately no second step. A quantisation or a nearest-neighbour snap would be a
// tolerance wearing a different word, and the point of this file is that the gate has none: the
// capture path removes the variance at its sources (`captureMode.ts` names each one) rather than
// absorbing it here. `assertOpaque` checks the single property this encoder assumes — that the
// capture context has no alpha channel, so every sample reads back fully opaque — instead of forcing
// it, because overwriting the channel would hide exactly the change worth knowing about.
//
// # Determinism of the compressor
//
// `deflateSync` with fixed options is a pure function for a given zlib build, and Node bundles its
// zlib. The row filter is fixed rather than adaptively chosen, so the byte stream does not depend on
// a heuristic that could change between releases.
//
// What this does *not* claim is stability across Node major versions: a different zlib would produce
// a different valid PNG of the same pixels. That surfaces as a manifest-checksum mismatch — loudly,
// telling whoever changed Node to re-run the capture — which is the correct failure. It never
// weakens the gate, because both sides of every comparison are encoded by the same process.

import { deflateSync, inflateSync, constants as zlibConstants } from 'node:zlib';

/** One framebuffer read: RGBA8, bottom-up, exactly as `readPixels` produced it. */
export interface RawFramebuffer {
  width: number;
  height: number;
  /** `width * height * 4` bytes, row 0 being the BOTTOM row of the image. */
  rgba: Buffer;
}

/** Bytes per pixel in every buffer this module handles. */
const CHANNELS = 4;

/**
 * PNG row filter used for every row.
 *
 * Paeth (type 4) compresses rendered imagery well and, unlike per-row adaptive selection, involves no
 * choice — so the encoded bytes are fixed by the pixels alone.
 */
const FILTER_PAETH = 4;

/** Reject a buffer whose length disagrees with its declared size before it becomes a corrupt PNG. */
function assertSized(fb: RawFramebuffer): void {
  if (
    fb.width <= 0 ||
    fb.height <= 0 ||
    !Number.isInteger(fb.width) ||
    !Number.isInteger(fb.height)
  ) {
    throw new Error(`canonical image: implausible framebuffer size ${fb.width}x${fb.height}`);
  }
  const expected = fb.width * fb.height * CHANNELS;
  if (fb.rgba.length !== expected) {
    throw new Error(
      `canonical image: ${fb.width}x${fb.height} needs ${expected} bytes of RGBA, got ${fb.rgba.length}`,
    );
  }
}

/**
 * Assert every sample is fully opaque, naming the first that is not.
 *
 * Capture mode requests a context without an alpha channel, and `readPixels` on such a context is
 * specified to return 1.0 for alpha. If that stops being true the saved PNGs become partly
 * transparent wherever geometry blended — water, precipitation — which looks like a rendering bug and
 * is really a context-configuration bug two files away.
 */
export function assertOpaque(fb: RawFramebuffer): void {
  for (let i = CHANNELS - 1; i < fb.rgba.length; i += CHANNELS) {
    if (fb.rgba[i] !== 0xff) {
      const px = (i - (CHANNELS - 1)) / CHANNELS;
      throw new Error(
        `canonical image: alpha ${fb.rgba[i]} at pixel (${px % fb.width}, ` +
          `${Math.floor(px / fb.width)} from the bottom). The capture context must be created with ` +
          `alpha: false — see WorldShowcase's gl props.`,
      );
    }
  }
}

/** The PNG Paeth predictor, specified in PNG 1.2 §6.6. */
function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
}

/**
 * Top-down RGBA scanlines, each prefixed with its filter byte.
 *
 * Filtering lives here rather than inside the compression step so the two halves of the format —
 * "which bytes describe the image" and "how are they compressed" — stay separately readable.
 */
function filteredScanlines(fb: RawFramebuffer): Buffer {
  const stride = fb.width * CHANNELS;
  const out = Buffer.allocUnsafe((stride + 1) * fb.height);
  // The previous row in *image* order, which is the row above in the output and below in the input.
  let prev = Buffer.alloc(stride);
  const cur = Buffer.allocUnsafe(stride);
  for (let y = 0; y < fb.height; y++) {
    // Row `y` from the top is row `height - 1 - y` from the bottom.
    const src = (fb.height - 1 - y) * stride;
    const dst = y * (stride + 1);
    fb.rgba.copy(cur, 0, src, src + stride);
    out[dst] = FILTER_PAETH;
    for (let x = 0; x < stride; x++) {
      const a = x >= CHANNELS ? cur[x - CHANNELS] : 0;
      const b = prev[x];
      const c = x >= CHANNELS ? prev[x - CHANNELS] : 0;
      out[dst + 1 + x] = (cur[x] - paeth(a, b, c)) & 0xff;
    }
    prev = Buffer.from(cur);
  }
  return out;
}

/** CRC-32 table, built once. */
const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

/** CRC-32 over `buf`, as PNG chunks require. */
function crc32(buf: Buffer): number {
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

/** One PNG chunk: length, type, payload, CRC. */
function chunk(type: string, payload: Buffer): Buffer {
  const head = Buffer.allocUnsafe(8);
  head.writeUInt32BE(payload.length, 0);
  head.write(type, 4, 'ascii');
  const crc = Buffer.allocUnsafe(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), payload])), 0);
  return Buffer.concat([head, payload, crc]);
}

/**
 * Encode a framebuffer read as the canonical PNG for that view.
 *
 * Deliberately minimal: signature, `IHDR`, one `IDAT`, `IEND`. No `tEXt`, no `tIME`, no `pHYs` — a
 * timestamp chunk alone would make every capture differ from every other capture, which is precisely
 * the failure this pipeline exists to make unreachable.
 *
 * @param fb - Bottom-up RGBA8, straight from `gl.readPixels`.
 * @returns The bytes to write to `map-views/<view>.png`, and to hash.
 */
export function encodeCanonicalPng(fb: RawFramebuffer): Buffer {
  assertSized(fb);
  assertOpaque(fb);

  const ihdr = Buffer.allocUnsafe(13);
  ihdr.writeUInt32BE(fb.width, 0);
  ihdr.writeUInt32BE(fb.height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: truecolour with alpha
  ihdr[10] = 0; // compression: deflate
  ihdr[11] = 0; // filtering: adaptive, per-row type byte
  ihdr[12] = 0; // no interlace

  const idat = deflateSync(filteredScanlines(fb), {
    level: 9,
    memLevel: 9,
    windowBits: 15,
    strategy: zlibConstants.Z_DEFAULT_STRATEGY,
  });

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', idat),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/**
 * Decode the pixels back out of a canonical PNG.
 *
 * For the encoder's own tests. An encoder asserted against a hand-written expected byte string tests
 * the assertion; `decode(encode(x)) === x` tests that the format is right. Handles exactly what
 * `encodeCanonicalPng` emits — 8-bit RGBA, non-interlaced, Paeth on every row — and throws on
 * anything else rather than growing into a general PNG reader.
 */
export function decodeCanonicalPng(png: Buffer): RawFramebuffer {
  const sig = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (png.length < 8 || !sig.every((b, i) => png[i] === b)) throw new Error('not a PNG');

  let off = 8;
  let width = 0;
  let height = 0;
  const parts: Buffer[] = [];
  while (off + 8 <= png.length) {
    const len = png.readUInt32BE(off);
    const type = png.toString('ascii', off + 4, off + 8);
    const body = png.subarray(off + 8, off + 8 + len);
    if (type === 'IHDR') {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      if (body[8] !== 8 || body[9] !== 6) throw new Error('expected 8-bit RGBA');
      if (body[12] !== 0) throw new Error('expected a non-interlaced PNG');
    } else if (type === 'IDAT') {
      parts.push(Buffer.from(body));
    }
    off += 12 + len;
  }

  const raw = inflateSync(Buffer.concat(parts));
  const stride = width * CHANNELS;
  const topDown = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const src = y * (stride + 1);
    if (raw[src] !== FILTER_PAETH) {
      throw new Error(`row ${y} used filter ${raw[src]}, expected Paeth (${FILTER_PAETH})`);
    }
    for (let x = 0; x < stride; x++) {
      const a = x >= CHANNELS ? topDown[y * stride + x - CHANNELS] : 0;
      const b = y > 0 ? topDown[(y - 1) * stride + x] : 0;
      const c = x >= CHANNELS && y > 0 ? topDown[(y - 1) * stride + x - CHANNELS] : 0;
      topDown[y * stride + x] = (raw[src + 1 + x] + paeth(a, b, c)) & 0xff;
    }
  }

  // Back to bottom-up, so `decodeCanonicalPng(encodeCanonicalPng(fb))` is comparable to `fb`.
  const rgba = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    topDown.copy(rgba, (height - 1 - y) * stride, y * stride, (y + 1) * stride);
  }
  return { width, height, rgba };
}
