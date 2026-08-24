import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, act, waitFor } from '@testing-library/react';
import { useState } from 'react';

const listen = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listen(...args),
}));

// Imported after the mock is registered so the hook picks up the stub.
const { useTauriEvent } = await import('../hooks/useTauriEvent');

type Handler = (event: { payload: unknown }) => void;

/** Records every subscription so a test can assert how many were made and fire into the live one. */
function trackedListen() {
  const unlistens: ReturnType<typeof vi.fn>[] = [];
  const handlers: Handler[] = [];
  listen.mockImplementation(async (_name: string, handler: Handler) => {
    handlers.push(handler);
    const unlisten = vi.fn();
    unlistens.push(unlisten);
    return unlisten;
  });
  return { unlistens, handlers };
}

beforeEach(() => {
  listen.mockReset();
});

describe('useTauriEvent', () => {
  it('subscribes once and keeps that subscription across re-renders', async () => {
    const { unlistens } = trackedListen();
    const seen: unknown[] = [];

    // The handler is written inline, exactly as every real call site does, so its identity changes
    // on every render. Naming it in the effect's dependency array — which this hook used to do —
    // made each render tear the listener down and re-register it.
    function Subject() {
      const [n, setN] = useState(0);
      useTauriEvent<number>('tick', (event) => {
        seen.push(event.payload);
      });
      return <button onClick={() => setN(n + 1)}>{n}</button>;
    }

    const { getByRole } = render(<Subject />);
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(1));

    for (let i = 0; i < 10; i++) {
      await act(async () => {
        getByRole('button').click();
      });
    }

    expect(getByRole('button').textContent).toBe('10');
    expect(listen).toHaveBeenCalledTimes(1);
    expect(unlistens[0]).not.toHaveBeenCalled();
  });

  it('dispatches into the latest render closure, not the one captured at subscribe time', async () => {
    const { handlers } = trackedListen();
    const seen: number[] = [];

    function Subject() {
      const [n, setN] = useState(0);
      useTauriEvent<string>('tick', () => {
        // Reads `n` from the current render. A subscription pinned to the first render would
        // report 0 forever.
        seen.push(n);
      });
      return <button onClick={() => setN(n + 1)}>{n}</button>;
    }

    const { getByRole } = render(<Subject />);
    await waitFor(() => expect(handlers).toHaveLength(1));

    await act(async () => {
      handlers[0]({ payload: 'a' });
    });
    await act(async () => {
      getByRole('button').click();
    });
    await act(async () => {
      handlers[0]({ payload: 'b' });
    });

    expect(seen).toEqual([0, 1]);
  });

  it('unsubscribes on unmount', async () => {
    const { unlistens } = trackedListen();

    function Subject() {
      useTauriEvent<number>('tick', () => {});
      return null;
    }

    const { unmount } = render(<Subject />);
    await waitFor(() => expect(unlistens).toHaveLength(1));

    unmount();
    await waitFor(() => expect(unlistens[0]).toHaveBeenCalledTimes(1));
  });

  it('drops a subscription that resolves after unmount instead of leaking it', async () => {
    const unlisten = vi.fn();
    let resolveListen: ((u: () => void) => void) | undefined;
    listen.mockImplementation(
      () => new Promise<() => void>((resolve) => { resolveListen = resolve; })
    );

    function Subject() {
      useTauriEvent<number>('tick', () => {});
      return null;
    }

    const { unmount } = render(<Subject />);
    unmount();

    // The await had not settled when cleanup ran; the hook must still release it.
    await act(async () => {
      resolveListen!(unlisten);
    });
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('routes a failed subscription to onError when one is given', async () => {
    listen.mockRejectedValue(new Error('no backend'));
    const onError = vi.fn();

    function Subject() {
      useTauriEvent<number>('tick', () => {}, onError);
      return null;
    }

    render(<Subject />);
    await waitFor(() => expect(onError).toHaveBeenCalledTimes(1));
    expect(String(onError.mock.calls[0][0])).toContain('no backend');
  });

  it('logs instead of throwing when a subscription fails and no onError is given', async () => {
    listen.mockRejectedValue(new Error('no backend'));
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});

    function Subject() {
      useTauriEvent<number>('tick', () => {});
      return null;
    }

    render(<Subject />);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    spy.mockRestore();
  });

  it('re-subscribes when the event name changes', async () => {
    trackedListen();

    function Subject({ name }: { name: string }) {
      useTauriEvent<number>(name, () => {});
      return null;
    }

    const { rerender } = render(<Subject name="a" />);
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(1));

    rerender(<Subject name="b" />);
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
    expect(listen.mock.calls.map((c) => c[0])).toEqual(['a', 'b']);
  });
});
