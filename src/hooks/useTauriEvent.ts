import { useEffect, useRef } from 'react';
import { listen, Event, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Subscribe to a Tauri event for the lifetime of the component.
 *
 * The handler is held in a ref rather than named in the effect's dependency array. That is the whole
 * point of this hook: every call site writes its handler inline, so the handler's identity changes
 * on every render. With `[eventName, handler]` deps — which is what this hook used to have — each
 * render tore the listener down and re-registered it. On the tick path that is a feedback loop: the
 * handler calls `setState`, the re-render produces a new handler identity, and the effect re-runs.
 * At the backend's 30 Hz emit rate that came to roughly 30 async `listen()`/`unlisten()` round trips
 * per second per event, each with a window in between where arriving events were dropped.
 *
 * `onError` is optional; without it a failed subscription is logged, which is what callers that only
 * want telemetry expect. Pass one to surface the failure in the UI.
 */
export function useTauriEvent<T>(
  eventName: string,
  handler: (event: Event<T>) => void,
  onError?: (err: unknown) => void
) {
  const handlerRef = useRef(handler);
  const onErrorRef = useRef(onError);

  // Refreshed after every commit, so the listener always dispatches into the current render's
  // closure without the subscription itself depending on it. Declared before the subscribing effect
  // so it is in place first on mount; `useRef(handler)` already seeds it with the first handler
  // anyway, and events can only arrive asynchronously after mount.
  useEffect(() => {
    handlerRef.current = handler;
    onErrorRef.current = onError;
  });

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | null = null;

    listen<T>(eventName, (event) => {
      if (active) {
        handlerRef.current(event);
      }
    })
      .then((u) => {
        if (active) {
          unlisten = u;
        } else {
          // The subscription resolved after this effect was already cleaned up. Drop it here or it
          // leaks into the next mount as a second live listener.
          u();
        }
      })
      .catch((err) => {
        if (!active) return;
        if (onErrorRef.current) {
          onErrorRef.current(err);
        } else {
          console.error(`Failed to listen to event ${eventName}:`, err);
        }
      });

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [eventName]);
}
