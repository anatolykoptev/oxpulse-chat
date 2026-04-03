import { describe, it, expect, vi, beforeEach } from 'vitest';
import { CallSignaling } from './signaling';

// Mock WebSocket
class MockWebSocket {
	static OPEN = 1;
	static instances: MockWebSocket[] = [];

	readyState = MockWebSocket.OPEN;
	onopen: (() => void) | null = null;
	onmessage: ((ev: { data: string }) => void) | null = null;
	onerror: (() => void) | null = null;
	onclose: (() => void) | null = null;
	sent: string[] = [];
	url: string;

	constructor(url: string) {
		this.url = url;
		MockWebSocket.instances.push(this);
		setTimeout(() => this.onopen?.(), 0);
	}

	send(data: string) {
		this.sent.push(data);
	}

	close() {
		this.readyState = 3;
	}
}

beforeEach(() => {
	MockWebSocket.instances = [];
	vi.stubGlobal('WebSocket', MockWebSocket);
});

describe('CallSignaling', () => {
	it('sends join message on connect', async () => {
		const callbacks = {
			onJoined: vi.fn(),
			onSignal: vi.fn(),
			onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(),
			onError: vi.fn(),
		};

		const sig = new CallSignaling(callbacks);
		sig.connect('http://localhost:8903', 'test-room');

		// Wait for onopen to fire
		await new Promise(r => setTimeout(r, 10));

		const ws = MockWebSocket.instances[0];
		expect(ws.url).toBe('ws://localhost:8903/ws/call/test-room');
		expect(ws.sent).toContainEqual(JSON.stringify({ type: 'join' }));
	});

	it('replaces http with ws in url', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('https://chat.example.com', 'room-123');
		await new Promise(r => setTimeout(r, 10));

		expect(MockWebSocket.instances[0].url).toBe('wss://chat.example.com/ws/call/room-123');
	});

	it('dispatches joined callback', async () => {
		const onJoined = vi.fn();
		const sig = new CallSignaling({
			onJoined, onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		const ws = MockWebSocket.instances[0];
		ws.onmessage?.({ data: JSON.stringify({ type: 'joined', polite: true }) });

		expect(onJoined).toHaveBeenCalledWith(true);
	});

	it('dispatches signal callback', async () => {
		const onSignal = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal, onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		const ws = MockWebSocket.instances[0];
		const payload = { type: 'offer', sdp: 'test-sdp' };
		ws.onmessage?.({ data: JSON.stringify({ type: 'signal', payload }) });

		expect(onSignal).toHaveBeenCalledWith(payload);
	});

	it('dispatches peer_joined callback', async () => {
		const onPeerJoined = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined,
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		MockWebSocket.instances[0].onmessage?.({ data: JSON.stringify({ type: 'peer_joined' }) });
		expect(onPeerJoined).toHaveBeenCalled();
	});

	it('dispatches peer_left callback', async () => {
		const onPeerLeft = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft, onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		MockWebSocket.instances[0].onmessage?.({ data: JSON.stringify({ type: 'peer_left' }) });
		expect(onPeerLeft).toHaveBeenCalled();
	});

	it('dispatches error callback', async () => {
		const onError = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError,
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		MockWebSocket.instances[0].onmessage?.({
			data: JSON.stringify({ type: 'error', message: 'Room is full' }),
		});
		expect(onError).toHaveBeenCalledWith('Room is full');
	});

	it('sends signal via sendSignal', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		sig.sendSignal({ type: 'offer', sdp: 'test' });

		const ws = MockWebSocket.instances[0];
		const signalMsg = ws.sent.find(s => JSON.parse(s).type === 'signal');
		expect(signalMsg).toBeDefined();
		expect(JSON.parse(signalMsg!).payload).toEqual({ type: 'offer', sdp: 'test' });
	});

	it('sends leave on close', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		sig.close();

		const ws = MockWebSocket.instances[0];
		const leaveMsg = ws.sent.find(s => JSON.parse(s).type === 'leave');
		expect(leaveMsg).toBeDefined();
	});

	it('ignores invalid JSON messages', async () => {
		const onSignal = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal, onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		// Should not throw
		MockWebSocket.instances[0].onmessage?.({ data: 'not-json{{{' });
		expect(onSignal).not.toHaveBeenCalled();
	});

	it('attempts reconnection on unexpected close', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));
		expect(MockWebSocket.instances).toHaveLength(1);

		// Simulate unexpected close
		MockWebSocket.instances[0].onclose?.();

		// Wait for reconnect backoff (1000ms)
		await new Promise(r => setTimeout(r, 1100));

		expect(MockWebSocket.instances).toHaveLength(2);
	});

	it('does not reconnect after explicit close', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		sig.close();

		// Simulate close event after user close
		MockWebSocket.instances[0].onclose = null; // close() nulls this

		await new Promise(r => setTimeout(r, 1200));

		// Should not have created a new WebSocket
		expect(MockWebSocket.instances).toHaveLength(1);
	});

	it('sends heartbeat pings', async () => {
		vi.useFakeTimers();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await vi.advanceTimersByTimeAsync(10);

		const ws = MockWebSocket.instances[0];
		const initialSent = ws.sent.length;

		// Advance past heartbeat interval (25s)
		await vi.advanceTimersByTimeAsync(25_000);

		const pings = ws.sent.slice(initialSent).filter(s => JSON.parse(s).type === 'ping');
		expect(pings).toHaveLength(1);

		vi.useRealTimers();
	});

	it('ignores pong messages', async () => {
		const onSignal = vi.fn();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal, onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await new Promise(r => setTimeout(r, 10));

		MockWebSocket.instances[0].onmessage?.({ data: JSON.stringify({ type: 'pong' }) });
		expect(onSignal).not.toHaveBeenCalled();
	});

	it('stops heartbeat on close', async () => {
		vi.useFakeTimers();
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room');
		await vi.advanceTimersByTimeAsync(10);

		const ws = MockWebSocket.instances[0];
		sig.close();

		const sentAfterClose = ws.sent.length;
		await vi.advanceTimersByTimeAsync(30_000);
		// No new pings after close (ws is closed so send would fail anyway,
		// but the interval should be cleared)
		expect(ws.sent.length).toBe(sentAfterClose);

		vi.useRealTimers();
	});

	it('encodes room id in url', async () => {
		const sig = new CallSignaling({
			onJoined: vi.fn(), onSignal: vi.fn(), onPeerJoined: vi.fn(),
			onPeerLeft: vi.fn(), onError: vi.fn(),
		});

		sig.connect('http://localhost', 'room with spaces');
		await new Promise(r => setTimeout(r, 10));

		expect(MockWebSocket.instances[0].url).toBe('ws://localhost/ws/call/room%20with%20spaces');
	});
});
