import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ── Mock collaborators before importing useCall ─────────────────────────────

const trackSpy = vi.fn();
vi.mock('./tracker', () => ({
	track: (...args: unknown[]) => trackSpy(...args),
}));

vi.mock('./sounds', () => ({
	unlockAudio: vi.fn(),
	playConnectSound: vi.fn(),
	playDisconnectSound: vi.fn(),
	startRinging: vi.fn(),
	stopRinging: vi.fn(),
}));

// Capture the onConnectionState callback so the test can fire 'connected' at will.
const createCallCapture: {
	onConnectionState: ((state: RTCPeerConnectionState) => void) | null;
} = { onConnectionState: null };

const fakeCallHandle = {
	handleSignal: vi.fn(async () => {}),
	addLocalStream: vi.fn(),
	replaceVideoTrack: vi.fn(async () => {}),
	replaceAudioTrack: vi.fn(async () => {}),
	restartNegotiation: vi.fn(),
	setVideoEnabled: vi.fn(),
	startScreenShare: vi.fn(async () => null),
	stopScreenShare: vi.fn(async () => {}),
	startStatsPolling: vi.fn(),
	stopStatsPolling: vi.fn(),
	getVerificationEmoji: vi.fn(() => ''),
	close: vi.fn(),
};

vi.mock('./webrtc', () => ({
	createCall: (opts: { onConnectionState: (s: RTCPeerConnectionState) => void }) => {
		createCallCapture.onConnectionState = opts.onConnectionState;
		return fakeCallHandle;
	},
	qualityLevel: vi.fn(() => 'good'),
}));

// Capture the signaling callbacks so the test can fire `onJoined` manually.
const signalingCapture: {
	onJoined: ((polite: boolean) => Promise<void> | void) | null;
} = { onJoined: null };

vi.mock('./signaling', () => ({
	CallSignaling: class {
		constructor(callbacks: { onJoined: (polite: boolean) => Promise<void> | void }) {
			signalingCapture.onJoined = callbacks.onJoined;
		}
		connect() {}
		sendSignal() {}
		close() {}
	},
	fetchTurnCredentials: vi.fn(async () => []),
}));

// ── Stub browser globals that useCall touches directly ──────────────────────

beforeEach(() => {
	trackSpy.mockClear();
	createCallCapture.onConnectionState = null;
	signalingCapture.onJoined = null;

	const fakeStream = {
		getAudioTracks: () => [],
		getVideoTracks: () => [],
		getTracks: () => [],
		addTrack: vi.fn(),
		removeTrack: vi.fn(),
	} as unknown as MediaStream;

	vi.stubGlobal('navigator', {
		mediaDevices: {
			getUserMedia: vi.fn(async () => fakeStream),
			enumerateDevices: vi.fn(async () => []),
		},
	});

	vi.stubGlobal('document', {
		referrer: '',
		visibilityState: 'visible',
		pictureInPictureEnabled: false,
		pictureInPictureElement: null,
		exitPictureInPicture: vi.fn(async () => {}),
	});

	vi.stubGlobal('window', {});
});

afterEach(() => {
	vi.unstubAllGlobals();
});

// ── Tests ───────────────────────────────────────────────────────────────────

describe('useCall — call_connected analytics guard', () => {
	async function importUseCall() {
		// Dynamic import so the module is re-evaluated fresh against the current mocks.
		return await import('./useCall.svelte');
	}

	it('tracks call_connected exactly once when ICE enters connected state twice', async () => {
		const { useCall } = await importUseCall();

		const call = useCall({
			get serverUrl() {
				return 'http://localhost:0';
			},
			get roomId() {
				return 'room-test';
			},
			onEnded: vi.fn(),
		});

		await call.init();

		// Drive the signaling handshake to the point where createCall fires.
		expect(signalingCapture.onJoined).not.toBeNull();
		await signalingCapture.onJoined!(true);

		// Now fire onConnectionState('connected') twice — simulating an ICE restart.
		expect(createCallCapture.onConnectionState).not.toBeNull();
		createCallCapture.onConnectionState!('connected');
		createCallCapture.onConnectionState!('connected');

		const connectedCalls = trackSpy.mock.calls.filter(
			(args) => args[0] === 'call_connected',
		);
		expect(connectedCalls).toHaveLength(1);
		expect(connectedCalls[0][1]).toBe('room-test');

		call.destroy();
	});

	it('still fires side effects (status update) on every connected transition', async () => {
		const { useCall } = await importUseCall();

		const call = useCall({
			get serverUrl() {
				return 'http://localhost:0';
			},
			get roomId() {
				return 'room-side-effects';
			},
			onEnded: vi.fn(),
		});

		await call.init();
		await signalingCapture.onJoined!(true);

		createCallCapture.onConnectionState!('connected');
		expect(call.status).toBe('connected');

		// Simulate a transient 'failed' then another 'connected' (ICE restart).
		createCallCapture.onConnectionState!('failed');
		expect(call.status).toBe('failed');

		createCallCapture.onConnectionState!('connected');
		// Status must re-reflect the current connection state, proving side effects run.
		expect(call.status).toBe('connected');

		// But analytics must still be exactly one call_connected.
		const connectedCalls = trackSpy.mock.calls.filter(
			(args) => args[0] === 'call_connected',
		);
		expect(connectedCalls).toHaveLength(1);

		call.destroy();
	});

	it('resets the guard on a fresh init so a second call still tracks once', async () => {
		const { useCall } = await importUseCall();

		const call = useCall({
			get serverUrl() {
				return 'http://localhost:0';
			},
			get roomId() {
				return 'room-second';
			},
			onEnded: vi.fn(),
		});

		// First call
		await call.init();
		await signalingCapture.onJoined!(true);
		createCallCapture.onConnectionState!('connected');
		createCallCapture.onConnectionState!('connected');

		// Second call on the same useCall instance (e.g. user navigates back)
		await call.init();
		await signalingCapture.onJoined!(true);
		createCallCapture.onConnectionState!('connected');

		const connectedCalls = trackSpy.mock.calls.filter(
			(args) => args[0] === 'call_connected',
		);
		expect(connectedCalls).toHaveLength(2);

		call.destroy();
	});
});
