import { CANONICAL_VIEW_CAMERAS, CANONICAL_VIEW_IDS } from './mapManifest';
import type { CanonicalPoint, CanonicalViewId } from './mapManifest';

// ---------------------------------------------------------------------------------------
// Deterministic capture mode for the canonical map views.
//
// # Why the app knows about this at all
//
// The eight canonical views are perspective renders of the real 3D scene. Producing them from
// anything other than the real renderer would be a picture of a different thing — an
// orthographic raster of worldgen data looks plausible and is evidence about nothing that ships.
// So the capture drives `landscape.html` in a real browser with real WebGL, which means the page
// has to be able to hold still: a fixed camera pose, a fixed clock, fixed weather, fixed quality.
//
// Left to itself the scene is deliberately never still. The day/night clock advances on a timer,
// weather drifts, the camera has momentum and head-bob. Two captures a second apart would differ,
// and a before/after pair that differs everywhere proves nothing about the change under review.
//
// # Why it cannot leak into normal use
//
// `readCaptureRequest` returns `null` unless the URL carries `capture=1`, and every effect it
// gates is downstream of that null. No environment variable, no build flag, no default-on path:
// an ordinary `landscape.html` visit takes exactly the code it took before. That matters more
// than convenience here, because a capture-mode flag that could switch itself on would make the
// shipped renderer's behaviour depend on a query string a user could stumble into.
// ---------------------------------------------------------------------------------------

/** Weather states `WorldWeather` accepts. */
export type CaptureWeather = 'clear' | 'rain' | 'snow' | 'fog';
/** Quality presets `WorldShowcase` accepts. */
export type CaptureQuality = 'low' | 'high';

/** A fully specified, reproducible shot. */
export interface CaptureRequest {
  view: CanonicalViewId;
  /** Camera pose in canonical bounds; convert with `canonicalCameraToRender`. */
  camera: CanonicalPoint;
  /** Hour of the day, 0..24. */
  timeOfDay: number;
  weather: CaptureWeather;
  quality: CaptureQuality;
}

/**
 * Defaults chosen so a view is *readable*, and stated here rather than in the harness so the
 * manifest, the harness and the app cannot disagree about what the canonical conditions are.
 *
 * 10:00 puts the sun high enough to light the terrain without the long raking shadows of dawn
 * that turn a biome-boundary shot into a study of shadow. Clear weather so nothing occludes the
 * subject. High quality because a low-quality capture drops every other flora instance, and
 * "half the trees are missing" is not a defect a reviewer should have to discount.
 */
export const CAPTURE_DEFAULT_TIME_OF_DAY = 10;
export const CAPTURE_DEFAULT_WEATHER: CaptureWeather = 'clear';
export const CAPTURE_DEFAULT_QUALITY: CaptureQuality = 'high';

const WEATHERS: readonly CaptureWeather[] = ['clear', 'rain', 'snow', 'fog'];
const QUALITIES: readonly CaptureQuality[] = ['low', 'high'];

function isCanonicalView(v: string): v is CanonicalViewId {
  return (CANONICAL_VIEW_IDS as readonly string[]).includes(v);
}

/**
 * Is this a capture page at all — without validating the rest of the request.
 *
 * Split out from `readCaptureRequest` because two callers need two different failure modes.
 * `WorldShowcase` needs the validating parse: a malformed `view=` must throw rather than quietly
 * shoot something else. `sceneClock.ts` needs only "must this page hold still", is consulted from
 * animated components on every frame, and must not be the thing that throws — the error belongs to
 * the parse, once, at the top.
 */
export function isCaptureRequested(search: string): boolean {
  return new URLSearchParams(search).get('capture') === '1';
}

/**
 * Parse a capture request from a URL query string, or `null` for a normal visit.
 *
 * Throws on a malformed request rather than falling back to a default. A capture that silently
 * substituted noon for an unparseable `t=` would produce an image labelled as one thing and shot
 * as another, which is the failure this whole area is being repaired for.
 */
export function readCaptureRequest(search: string): CaptureRequest | null {
  const params = new URLSearchParams(search);
  if (params.get('capture') !== '1') return null;

  const view = params.get('view') ?? '';
  if (!isCanonicalView(view)) {
    throw new Error(
      `capture: "view" must be one of ${CANONICAL_VIEW_IDS.join(', ')} (got ${JSON.stringify(view)})`,
    );
  }

  const rawTime = params.get('t');
  const timeOfDay = rawTime === null ? CAPTURE_DEFAULT_TIME_OF_DAY : Number(rawTime);
  if (!Number.isFinite(timeOfDay) || timeOfDay < 0 || timeOfDay >= 24) {
    throw new Error(`capture: "t" must be an hour in [0, 24) (got ${JSON.stringify(rawTime)})`);
  }

  const rawWeather = params.get('weather') ?? CAPTURE_DEFAULT_WEATHER;
  if (!WEATHERS.includes(rawWeather as CaptureWeather)) {
    throw new Error(`capture: "weather" must be one of ${WEATHERS.join(', ')} (got ${rawWeather})`);
  }

  const rawQuality = params.get('quality') ?? CAPTURE_DEFAULT_QUALITY;
  if (!QUALITIES.includes(rawQuality as CaptureQuality)) {
    throw new Error(`capture: "quality" must be one of ${QUALITIES.join(', ')} (got ${rawQuality})`);
  }

  return {
    view,
    camera: CANONICAL_VIEW_CAMERAS[view],
    timeOfDay,
    weather: rawWeather as CaptureWeather,
    quality: rawQuality as CaptureQuality,
  };
}

/** Name of the flag the harness polls to know the scene has settled. */
export const CAPTURE_READY_FLAG = '__animaCaptureReady';

// ---- the acceptance criterion: exact byte identity ---------------------------------------
//
// **A canonical view captured twice from two equivalently clean loads must produce byte-identical
// saved PNGs, and therefore one SHA-256.** Not "identical within a bound". Identical.
//
// An earlier pass measured 3–12 pixels of 921 600 differing by one level of 255 between two runs,
// attributed it to the GPU's MSAA resolve, and published two constants — a maximum channel delta and
// a maximum differing fraction — as the gate. Both are gone, along with the pixel-difference
// comparator that read them. The reasoning was wrong in a specific way worth recording, because it is
// the reasoning any future measurement of a stubborn last-bit difference will suggest again:
//
//   * "the hardware is nondeterministic" is a claim nobody established. What was established is that *this
//     pipeline* was nondeterministic. MSAA resolve order is one candidate cause among several, and
//     the others — an undefined drawing-buffer read, a compositor recomposite, a screenshot encoder —
//     were all still in the path when the measurement was taken.
//   * a tolerance cannot distinguish "the GPU rounded" from "a shader constant moved by one ULP", so
//     the bound that admits the first silently admits the second forever.
//   * and it is unfalsifiable in the direction that matters: a gate that passes at 12 differing
//     pixels tells you nothing when the next change makes it 40.
//
// So the variance is removed at its sources instead, and every source is named:
//
//   MSAA resolve          `antialias: false` under capture. There is no resolve to be
//                         order-dependent. The images are aliased where they were smoothed; that is
//                         a visible, honest cost of a gate that can be held.
//   dithering             `DITHER` disabled on the capture context. It is enabled by default and is
//                         explicitly allowed to be implementation-defined.
//   undefined buffer      `preserveDrawingBuffer: true` under capture, so reading the drawing buffer
//                         after the frame is a defined operation rather than a race with compositing.
//   alpha compositing     `alpha: false` under capture: no premultiply, no blend against the page,
//                         and `readPixels` returns a fully opaque frame by specification.
//   a moving scene        `sceneClock.ts` — fixed elapsed time, fixed frame delta, seeded randomness,
//                         snapped easing.
//   *when* the frame is   the loop is stopped at a fixed frame count and exactly one render is issued
//     read                afterwards, so the buffer being read is the product of a known number of
//                         renders rather than of however many fitted in before the screenshot.
//   the encoder           the harness reads back the framebuffer itself and encodes the PNG with a
//                         fixed filter and fixed deflate settings (`tests/e2e/canonicalImage.ts`),
//                         so the saved bytes are a function of the pixels and nothing else.
//
// If a residual difference ever survives all of that, the answer is to find and name the source. It
// is not to reintroduce a bound. `canonical_views.spec.ts` shoots every view twice from two fresh
// browser contexts, encodes both through the full saved-output pipeline, and requires the two
// SHA-256s to be equal before it writes anything.

/**
 * Rendered frames to wait for before a capture is considered stable.
 *
 * 90 is ~1.5 s at 60 fps. Measured need is lower — terrain mesh, instanced flora upload and the
 * first shadow pass are all done well inside it — but a capture that is occasionally taken one
 * frame early is worse than one that always takes an extra second, because the resulting image
 * looks fine and differs from its own previous run.
 *
 * The count is what makes the frame deterministic, not the wait: the loop stops on exactly this
 * frame, so "settled" is a fixed number of renders rather than a duration.
 */
export const CAPTURE_SETTLE_FRAMES = 90;
