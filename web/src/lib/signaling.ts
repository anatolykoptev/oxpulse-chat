export interface SignalingCallbacks {
	onJoined: (polite: boolean) => void;
	onSignal: (payload: SignalMessage) => void;
	onPeerJoined: () => void;
	onPeerLeft: () => void;
	onError: (message: string) => void;
}

export interface SignalMessage {
	type: string;
	sdp?: string;
	candidate?: string;
	sdpMid?: string | null;
}

const MAX_RECONNECTS = 3;
const BACKOFF = [1000, 2000, 4000];

export class CallSignaling {
	private ws: WebSocket | null = null;
	private callbacks: SignalingCallbacks;
	private serverUrl = '';
	private roomId = '';
	private closed = false;
	private reconnectAttempts = 0;
	private heartbeatInterval: ReturnType<typeof setInterval> | null = null;

	constructor(callbacks: SignalingCallbacks) {
		this.callbacks = callbacks;
	}

	connect(serverUrl: string, roomId: string): void {
		this.serverUrl = serverUrl;
		this.roomId = roomId;
		this.closed = false;
		this.reconnectAttempts = 0;
		this.doConnect();
	}

	private startHeartbeat(): void {
		this.stopHeartbeat();
		this.heartbeatInterval = setInterval(() => {
			if (this.ws?.readyState === WebSocket.OPEN) {
				this.ws.send(JSON.stringify({ type: 'ping' }));
			}
		}, 25_000);
	}

	private stopHeartbeat(): void {
		if (this.heartbeatInterval) {
			clearInterval(this.heartbeatInterval);
			this.heartbeatInterval = null;
		}
	}

	private doConnect(): void {
		const wsUrl = this.serverUrl.replace(/^http/, 'ws') + '/ws/call/' + encodeURIComponent(this.roomId);
		this.ws = new WebSocket(wsUrl);
		this.ws.onopen = () => {
			this.reconnectAttempts = 0;
			this.ws!.send(JSON.stringify({ type: 'join' }));
			this.startHeartbeat();
		};
		this.ws.onmessage = (ev) => {
			let data: Record<string, unknown>;
			try { data = JSON.parse(ev.data); } catch { return; }
			const type = data.type as string;
			if (type === 'pong') return; // heartbeat response
			if (type === 'joined') this.callbacks.onJoined(data.polite as boolean);
			else if (type === 'signal') this.callbacks.onSignal(data.payload as SignalMessage);
			else if (type === 'peer_joined') this.callbacks.onPeerJoined();
			else if (type === 'peer_left') this.callbacks.onPeerLeft();
			else if (type === 'error') this.callbacks.onError(data.message as string);
		};
		this.ws.onerror = () => {};
		this.ws.onclose = () => {
			this.ws = null;
			if (!this.closed && this.reconnectAttempts < MAX_RECONNECTS) {
				const delay = BACKOFF[this.reconnectAttempts] ?? 4000;
				this.reconnectAttempts++;
				setTimeout(() => {
					if (!this.closed) this.doConnect();
				}, delay);
			} else if (!this.closed) {
				this.callbacks.onError('Connection lost');
			}
		};
	}

	sendSignal(payload: unknown): void {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(JSON.stringify({ type: 'signal', payload }));
		}
	}

	close(): void {
		this.closed = true;
		this.stopHeartbeat();
		if (this.ws) {
			if (this.ws.readyState === WebSocket.OPEN) {
				this.ws.send(JSON.stringify({ type: 'leave' }));
			}
			this.ws.onclose = null;
			this.ws.close();
			this.ws = null;
		}
	}
}

const STUN_FALLBACK: RTCIceServer[] = [{ urls: 'stun:stun.l.google.com:19302' }];

export async function fetchTurnCredentials(serverUrl: string): Promise<RTCIceServer[]> {
	try {
		const r = await fetch(serverUrl + '/api/turn-credentials', { method: 'POST' });
		if (!r.ok) return STUN_FALLBACK;
		const d = await r.json();
		return d.ice_servers ?? d.iceServers ?? STUN_FALLBACK;
	} catch {
		return STUN_FALLBACK;
	}
}
