function getDeviceId(): string {
  const key = 'ox_did';
  let id = localStorage.getItem(key);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(key, id);
  }
  return id;
}

let queue: Array<{ e: string; r?: string; d?: Record<string, unknown> }> = [];
let timer: ReturnType<typeof setTimeout> | null = null;

export function track(
  eventType: string,
  roomId?: string,
  data?: Record<string, unknown>,
) {
  queue.push({ e: eventType, r: roomId, d: data });
  if (!timer) timer = setTimeout(flush, 2000);
}

function flush() {
  timer = null;
  if (queue.length === 0) return;
  const payload = JSON.stringify({ did: getDeviceId(), src: location.hostname, events: queue });
  queue = [];
  if (navigator.sendBeacon) {
    navigator.sendBeacon(
      '/api/event',
      new Blob([payload], { type: 'application/json' }),
    );
  } else {
    fetch('/api/event', {
      method: 'POST',
      body: payload,
      headers: { 'Content-Type': 'application/json' },
      keepalive: true,
    }).catch(() => {});
  }
}

if (typeof window !== 'undefined') {
  window.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flush();
  });
}
