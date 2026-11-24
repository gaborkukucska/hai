// <!-- # START OF FILE hainet-portal/src/lib/dmNotifications.tsx -->
// NOSLOP_HUB_DM_VIEW_V1 — unread counts and desktop notifications for DMs that reach the Home HUB.
//
// The Hub opens the owner's DMs as they arrive and records each incoming one as an event (`hub_dm_events`,
// hainet-noslop hub::dms). This provider polls those events, keeps an unread count per conversation (what
// this browser has not opened yet, remembered in localStorage) and, when the "Enable Notifications"
// setting is on and the browser allows it, shows a desktop notification. Browsers only allow notifications
// on a secure origin: http://localhost, or the .onion in Tor Browser; on a plain LAN http address the
// badges still work.

import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from './tauri';

export interface DmEvent {
  seq: number;
  dm_id: string;
  peer: string;
  sender: string;
  preview: string;
  at: number;
}

interface DmNotificationsValue {
  /** Unread incoming DMs per conversation (contact public key). */
  unread: Record<string, number>;
  totalUnread: number;
  /** Marks a conversation read up to the newest event. */
  markRead: (peer: string) => void;
  /** The conversation on screen (no notification for it while the page is visible), or null. */
  setActivePeer: (peer: string | null) => void;
  /** Whether this browser can show desktop notifications here, and what it currently allows. */
  notificationSupport: 'unsupported' | NotificationPermission;
  /** Asks the browser for permission (call from a click). */
  requestPermission: () => Promise<void>;
}

const POLL_MS = 5000;
const KEY_BASELINE = 'hainet.dm.baselineSeq';
const KEY_NOTIFIED = 'hainet.dm.notifiedSeq';
const KEY_READ = 'hainet.dm.readSeq';

const Ctx = createContext<DmNotificationsValue | null>(null);

function loadNumber(key: string): number | null {
  try {
    const v = localStorage.getItem(key);
    return v === null ? null : Number(v) || 0;
  } catch { return null; }
}
function saveNumber(key: string, n: number) {
  try { localStorage.setItem(key, String(n)); } catch { /* storage unavailable: counts reset on reload */ }
}
function loadReadMap(): Record<string, number> {
  try { return JSON.parse(localStorage.getItem(KEY_READ) || '{}') || {}; } catch { return {}; }
}
function saveReadMap(m: Record<string, number>) {
  try { localStorage.setItem(KEY_READ, JSON.stringify(m)); } catch { /* see above */ }
}

function support(): 'unsupported' | NotificationPermission {
  if (typeof window === 'undefined' || !('Notification' in window) || !window.isSecureContext) return 'unsupported';
  return Notification.permission;
}

function beep() {
  try {
    const AudioCtx = (window as any).AudioContext || (window as any).webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = 880;
    gain.gain.setValueAtTime(0.08, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.25);
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.25);
    osc.onended = () => ctx.close();
  } catch { /* sound is optional */ }
}

export function DmNotificationsProvider({ children }: { children: React.ReactNode }) {
  const navigate = useNavigate();
  const [events, setEvents] = useState<DmEvent[]>([]);
  const [readSeq, setReadSeq] = useState<Record<string, number>>(loadReadMap);
  const [baseline, setBaseline] = useState<number | null>(() => loadNumber(KEY_BASELINE));
  const [perm, setPerm] = useState<'unsupported' | NotificationPermission>(support);
  const lastSeq = useRef<number>(0);
  const activePeer = useRef<string | null>(null);
  const names = useRef<Record<string, string>>({});
  const prefs = useRef<{ notify: boolean; sound: boolean }>({ notify: true, sound: false });

  // Contact names and the notification settings (both change rarely).
  useEffect(() => {
    let stop = false;
    const load = async () => {
      try {
        const res = await invoke<{ peers: any[] }>('get_mesh_peers');
        const m: Record<string, string> = {};
        (res?.peers || []).forEach((p: any) => { if (p?.public_key) m[p.public_key] = p.handle || 'Contact'; });
        if (!stop) names.current = m;
      } catch { /* keep the previous names */ }
      try {
        const s = await invoke<any>('get_settings');
        if (!stop && s) prefs.current = { notify: s.enable_notifications !== false, sound: !!s.enable_sound };
      } catch { /* keep defaults */ }
    };
    load();
    const t = setInterval(load, 60000);
    return () => { stop = true; clearInterval(t); };
  }, []);

  const notify = useCallback((fresh: DmEvent[]) => {
    const visible = typeof document !== 'undefined' && !document.hidden;
    const shown = fresh.filter(e => !(visible && activePeer.current === e.peer));
    if (shown.length === 0) return;
    if (prefs.current.sound) beep();
    if (!prefs.current.notify || support() !== 'granted') return;
    for (const e of shown.slice(-3)) { // at most a few at once after being away
      try {
        const n = new Notification(names.current[e.peer] || 'New message', { body: e.preview || 'New message', tag: e.dm_id });
        n.onclick = () => { window.focus(); navigate(`/chat?peer=${encodeURIComponent(e.peer)}`); n.close(); };
      } catch { /* some browsers only allow notifications from a service worker */ }
    }
  }, [navigate]);

  // Poll the Hub's DM events.
  useEffect(() => {
    let stop = false;
    const poll = async () => {
      try {
        let base = baseline;
        if (base === null) {
          // First run in this browser: everything already on the Hub counts as read, nothing pops up.
          const first = await invoke<{ events: DmEvent[]; latest: number }>('hub_dm_events', { after: 0, limit: 1 });
          base = first?.latest || 0;
          saveNumber(KEY_BASELINE, base);
          saveNumber(KEY_NOTIFIED, base);
          if (!stop) setBaseline(base);
        }
        let after = Math.max(lastSeq.current, base);
        const fresh: DmEvent[] = [];
        for (let page = 0; page < 5; page++) {
          const res = await invoke<{ events: DmEvent[]; latest: number }>('hub_dm_events', { after, limit: 500 });
          const evs = res?.events || [];
          fresh.push(...evs);
          if (evs.length === 0 || after >= (res?.latest || 0)) break;
          after = evs[evs.length - 1].seq;
        }
        if (stop || fresh.length === 0) return;
        lastSeq.current = fresh[fresh.length - 1].seq;
        setEvents(prev => [...prev, ...fresh].slice(-1000));
        const notified = loadNumber(KEY_NOTIFIED) ?? base;
        const toNotify = fresh.filter(e => e.seq > notified);
        if (toNotify.length > 0) {
          saveNumber(KEY_NOTIFIED, toNotify[toNotify.length - 1].seq);
          notify(toNotify);
        }
        // The conversation on screen is read as messages arrive.
        const open = activePeer.current;
        if (open && !document.hidden && fresh.some(e => e.peer === open)) {
          setReadSeq(prev => { const m = { ...prev, [open]: lastSeq.current }; saveReadMap(m); return m; });
        }
      } catch { /* the Hub is restarting or the session ended: try again on the next tick */ }
    };
    poll();
    const t = setInterval(poll, POLL_MS);
    return () => { stop = true; clearInterval(t); };
  }, [baseline, notify]);

  const unread = useMemo(() => {
    const out: Record<string, number> = {};
    const base = baseline ?? Number.MAX_SAFE_INTEGER;
    for (const e of events) {
      if (e.seq > Math.max(base, readSeq[e.peer] || 0)) out[e.peer] = (out[e.peer] || 0) + 1;
    }
    return out;
  }, [events, readSeq, baseline]);
  const totalUnread = useMemo(() => Object.values(unread).reduce((a, b) => a + b, 0), [unread]);

  useEffect(() => {
    const plain = document.title.replace(/^\(\d+\)\s*/, '');
    document.title = totalUnread > 0 ? `(${totalUnread}) ${plain}` : plain;
  }, [totalUnread]);

  const markRead = useCallback((peer: string) => {
    setReadSeq(prev => {
      const upTo = Math.max(lastSeq.current, prev[peer] || 0);
      if ((prev[peer] || 0) === upTo) return prev;
      const m = { ...prev, [peer]: upTo };
      saveReadMap(m);
      return m;
    });
  }, []);

  const setActivePeer = useCallback((peer: string | null) => {
    activePeer.current = peer;
    if (peer) markRead(peer);
  }, [markRead]);

  const requestPermission = useCallback(async () => {
    if (support() === 'unsupported') { setPerm('unsupported'); return; }
    try { setPerm(await Notification.requestPermission()); } catch { setPerm(support()); }
  }, []);

  const value = useMemo<DmNotificationsValue>(() => ({
    unread, totalUnread, markRead, setActivePeer, notificationSupport: perm, requestPermission,
  }), [unread, totalUnread, markRead, setActivePeer, perm, requestPermission]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useDmNotifications(): DmNotificationsValue {
  const v = useContext(Ctx);
  if (!v) {
    return {
      unread: {}, totalUnread: 0, markRead: () => {}, setActivePeer: () => {},
      notificationSupport: 'unsupported', requestPermission: async () => {},
    };
  }
  return v;
}
// <!-- # END OF FILE hainet-portal/src/lib/dmNotifications.tsx -->
