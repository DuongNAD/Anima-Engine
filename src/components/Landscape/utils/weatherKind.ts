// The four weathers, in one place.
//
// The union was written out four times — the legacy `Weather` component's prop, the modern
// `WorldWeather`'s exported `WeatherKind`, `LandscapeShowcase`'s state, and the capture request's
// schema — and the controls overlay that drives two of them declared its callback as `(w: string)`.
// The overlay is not wrong to be permissive about what it *displays*: an adversarial test renders it
// with a stale `weather="custom-weather"` and expects to see that value, which is what a corrupted
// preference looks like. It was wrong to be permissive about what it *emits*, because the consumer
// then had to cast inside `setWeather(w)` to put a `string` into a union-typed state.
//
// So: one union, and one narrowing function at the boundary where a `<select>`'s `string` becomes a
// weather again.

/** Every weather the scene knows how to draw. The list is the source of the union below. */
export const WEATHER_KINDS = ['clear', 'rain', 'snow', 'fog'] as const;

/** One of the four weathers. */
export type WeatherKind = (typeof WEATHER_KINDS)[number];

/** `value` as a weather, or `null` when it is not one. */
export function asWeatherKind(value: string): WeatherKind | null {
  return (WEATHER_KINDS as readonly string[]).includes(value) ? (value as WeatherKind) : null;
}
