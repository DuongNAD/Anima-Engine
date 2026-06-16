import { useEffect } from 'react';
import { listen, Event } from '@tauri-apps/api/event';

export function useTauriEvent<T>(eventName: string, handler: (event: Event<T>) => void) {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let active = true;

    async function setupListener() {
      try {
        const u = await listen<T>(eventName, (event) => {
          if (active) {
            handler(event);
          }
        });
        if (!active) {
          u();
        } else {
          unlisten = u;
        }
      } catch (err) {
        console.error(`Failed to listen to event ${eventName}:`, err);
      }
    }

    setupListener();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [eventName, handler]);
}
