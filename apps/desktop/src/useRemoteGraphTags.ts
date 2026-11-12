import {useEffect, useState} from 'react';
import {api} from './api';
import type {RemoteGraphTags} from './contracts';

type State = {key: string; snapshot: RemoteGraphTags | null; error: boolean};

// Native cache is bounded and keyed by repository identity, fetch URL and refs.
// Keep network work off the graph load path and stop polling while hidden/busy.
export function useRemoteGraphTags(id: string, fingerprint: string, paused: boolean) {
  const [state, setState] = useState<State>({key: '', snapshot: null, error: false});
  const key = JSON.stringify([id, fingerprint]);
  useEffect(() => {
    if (paused) return;
    let live = true, reading = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = async () => {
      if (!live || reading || document.hidden) return;
      if (document.querySelector('dialog[open]')) {
        timer = setTimeout(() => void read(), 1000);
        return;
      }
      reading = true;
      try {
        const snapshot = await api.remoteGraphTags(id);
        if (live) setState({key, snapshot, error: false});
      } catch {
        if (live) setState(previous => ({key, snapshot: previous.key === key ? previous.snapshot : null, error: true}));
      } finally {
        reading = false;
        if (live && !document.hidden) timer = setTimeout(() => void read(), 60000);
      }
    };
    const visible = () => {
      clearTimeout(timer);
      if (!document.hidden) timer = setTimeout(() => void read(), 500);
    };
    timer = setTimeout(() => void read(), 500);
    document.addEventListener('visibilitychange', visible);
    return () => {
      live = false;
      clearTimeout(timer);
      document.removeEventListener('visibilitychange', visible);
    };
  }, [id, key, paused]);
  return state.key === key ? state : {key, snapshot: null, error: false};
}
