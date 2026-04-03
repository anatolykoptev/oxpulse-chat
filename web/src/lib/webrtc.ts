import type { SignalMessage } from './signaling';

// ── Constants ────────────────────────────────────────────────────────────────

/** Stats polling interval for quality monitoring. */
const STATS_POLL_INTERVAL_MS = 3000;

/** RTT thresholds (ms) for quality classification. */
const QUALITY_RTT_POOR_THRESHOLD = 400;
const QUALITY_RTT_FAIR_THRESHOLD = 150;

/** Packet-loss ratio thresholds for quality classification. */
const QUALITY_LOSS_POOR_THRESHOLD = 0.1;
const QUALITY_LOSS_FAIR_THRESHOLD = 0.03;

/** Max bitrates for senders (bps). */
const VIDEO_MAX_BITRATE = 1_500_000;
const VIDEO_BITRATE_FAIR = 800_000;
const VIDEO_BITRATE_POOR = 300_000;
const AUDIO_MAX_BITRATE = 64_000;
const AUDIO_ONLY_BITRATE = 96_000;

/** Jitter buffer targets (ms). */
const JITTER_BUFFER_TARGET_MS = 50;
const JITTER_BUFFER_AUDIO_ONLY_MS = 20;

/** ICE candidate pool size for the peer connection. */
const ICE_CANDIDATE_POOL_SIZE = 1;

/** Number of emoji symbols in the verification code. */
const VERIFICATION_EMOJI_COUNT = 4;

/** RTT is reported in seconds by WebRTC; multiplier to convert to ms. */
const RTT_TO_MS = 1000;

/** Bits per byte, used in bitrate calculation. */
const BITS_PER_BYTE = 8;

/** Divisor to convert bps to kbps in bitrate display. */
const KBPS_DIVISOR = 1000;

// ── Types ────────────────────────────────────────────────────────────────────

export interface QualityStats {
	rtt: number;
	packetLoss: number;
	bitrate: number;
}

export type QualityLevel = 'good' | 'fair' | 'poor';

export function qualityLevel(stats: QualityStats): QualityLevel {
	if (stats.rtt > QUALITY_RTT_POOR_THRESHOLD || stats.packetLoss > QUALITY_LOSS_POOR_THRESHOLD) return 'poor';
	if (stats.rtt > QUALITY_RTT_FAIR_THRESHOLD || stats.packetLoss > QUALITY_LOSS_FAIR_THRESHOLD) return 'fair';
	return 'good';
}

export interface CallOptions {
	iceServers: RTCIceServer[];
	polite: boolean;
	onRemoteStream: (stream: MediaStream) => void;
	onConnectionState: (state: RTCPeerConnectionState) => void;
	sendSignal: (msg: unknown) => void;
	onQualityUpdate?: (stats: QualityStats) => void;
}

export interface CallHandle {
	handleSignal: (msg: SignalMessage) => Promise<void>;
	addLocalStream: (stream: MediaStream) => void;
	replaceVideoTrack: (track: MediaStreamTrack) => Promise<void>;
	replaceAudioTrack: (track: MediaStreamTrack) => Promise<void>;
	restartNegotiation: () => void;
	setVideoEnabled: (enabled: boolean) => void;
	startScreenShare: () => Promise<MediaStream | null>;
	stopScreenShare: (cameraTrack: MediaStreamTrack) => Promise<void>;
	startStatsPolling: () => void;
	stopStatsPolling: () => void;
	getVerificationEmoji: () => string;
	close: () => void;
}

/** Prefer H.264 (HW accel) for lowest encode latency, then VP8. */
function setCodecPreferences(pc: RTCPeerConnection) {
	try {
		for (const tr of pc.getTransceivers()) {
			if (tr.receiver.track?.kind === 'video' || tr.sender.track?.kind === 'video') {
				const codecs = RTCRtpReceiver.getCapabilities('video')?.codecs;
				if (!codecs || !tr.setCodecPreferences) continue;
				const h264 = codecs.filter(c => c.mimeType === 'video/H264');
				const vp8 = codecs.filter(c => c.mimeType === 'video/VP8');
				const rest = codecs.filter(c => c.mimeType !== 'video/H264' && c.mimeType !== 'video/VP8');
				tr.setCodecPreferences([...h264, ...vp8, ...rest]);
			}
			if (tr.receiver.track?.kind === 'audio' || tr.sender.track?.kind === 'audio') {
				const codecs = RTCRtpReceiver.getCapabilities('audio')?.codecs;
				if (!codecs || !tr.setCodecPreferences) continue;
				const opus = codecs.filter(c => c.mimeType === 'audio/opus');
				const rest = codecs.filter(c => c.mimeType !== 'audio/opus');
				tr.setCodecPreferences([...opus, ...rest]);
			}
		}
	} catch { /* codec preferences not supported */ }
}

/** Set sender parameters for low latency: maintain framerate, cap bitrate. */
function configureSenderParams(pc: RTCPeerConnection) {
	for (const sender of pc.getSenders()) {
		const params = sender.getParameters();
		if (!params.encodings?.length) continue;
		for (const enc of params.encodings) {
			if (sender.track?.kind === 'video') {
				enc.maxBitrate = VIDEO_MAX_BITRATE; // cap to avoid congestion
				(enc as any).degradationPreference = 'maintain-framerate';
				(enc as any).networkPriority = 'high';
			} else if (sender.track?.kind === 'audio') {
				enc.maxBitrate = AUDIO_MAX_BITRATE; // Opus
				(enc as any).networkPriority = 'high';
			}
		}
		sender.setParameters(params).catch(() => {});
	}
}

/** Set low jitter buffer target on receivers for minimal playout delay. */
function configureReceiverJitter(pc: RTCPeerConnection) {
	for (const receiver of pc.getReceivers()) {
		if ('jitterBufferTarget' in receiver) {
			(receiver as any).jitterBufferTarget = JITTER_BUFFER_TARGET_MS;
		}
	}
}

export function createCall(options: CallOptions): CallHandle {
	const { iceServers, polite, onRemoteStream, onConnectionState, sendSignal } = options;
	let makingOffer = false;
	let isSettingRemoteAnswerPending = false;
	let ignoreOffer = false;
	const pendingCandidates: RTCIceCandidateInit[] = [];
	let lowLatencyConfigured = false;

	const pc = new RTCPeerConnection({
		iceServers,
		iceTransportPolicy: 'relay',
		bundlePolicy: 'max-bundle',
		rtcpMuxPolicy: 'require',
		iceCandidatePoolSize: ICE_CANDIDATE_POOL_SIZE,
	});

	/** Apply low-latency tuning once connection is established. */
	function applyLowLatencyConfig() {
		if (lowLatencyConfigured) return;
		lowLatencyConfigured = true;
		configureSenderParams(pc);
		configureReceiverJitter(pc);
	}

	async function makeOffer() {
		try {
			makingOffer = true;
			await pc.setLocalDescription();
			sendSignal({ type: pc.localDescription!.type, sdp: pc.localDescription!.sdp });
		} catch (e) {
			console.error('makeOffer error:', e);
		} finally {
			makingOffer = false;
		}
	}

	pc.onnegotiationneeded = () => { makeOffer(); };

	pc.onicecandidate = (ev) => {
		if (ev.candidate) {
			sendSignal({ type: 'candidate', candidate: ev.candidate.candidate, sdpMid: ev.candidate.sdpMid });
		}
	};

	pc.ontrack = (ev) => {
		if (ev.streams[0]) onRemoteStream(ev.streams[0]);
		// Set codec preferences after tracks are added
		setCodecPreferences(pc);
		configureReceiverJitter(pc);
	};

	// ICE restart on disconnection (e.g. network switch)
	pc.oniceconnectionstatechange = () => {
		if (pc.iceConnectionState === 'disconnected' || pc.iceConnectionState === 'failed') {
			pc.restartIce();
			makeOffer();
		}
	};

	pc.onconnectionstatechange = () => {
		if (pc.connectionState === 'connected') applyLowLatencyConfig();
		onConnectionState(pc.connectionState);
	};

	async function flushPendingCandidates() {
		while (pendingCandidates.length > 0) {
			const c = pendingCandidates.shift()!;
			try { await pc.addIceCandidate(c); } catch { /* ignore stale candidates */ }
		}
	}

	async function handleSignal(msg: SignalMessage): Promise<void> {
		try {
			if (msg.type === 'offer' || msg.type === 'answer') {
				const readyForOffer = !makingOffer &&
					(pc.signalingState === 'stable' || isSettingRemoteAnswerPending);
				const offerCollision = msg.type === 'offer' && !readyForOffer;

				ignoreOffer = !polite && offerCollision;
				if (ignoreOffer) return;

				isSettingRemoteAnswerPending = msg.type === 'answer';
				await pc.setRemoteDescription({ type: msg.type as RTCSdpType, sdp: msg.sdp });
				isSettingRemoteAnswerPending = false;

				await flushPendingCandidates();

				if (msg.type === 'offer') {
					await pc.setLocalDescription();
					sendSignal({ type: pc.localDescription!.type, sdp: pc.localDescription!.sdp });
				}
			} else if (msg.type === 'candidate') {
				const candidate: RTCIceCandidateInit = {
					candidate: msg.candidate,
					sdpMid: msg.sdpMid ?? undefined,
				};
				if (!pc.remoteDescription) {
					pendingCandidates.push(candidate);
				} else {
					try {
						await pc.addIceCandidate(candidate);
					} catch (e) {
						if (!ignoreOffer) console.error('addIceCandidate error:', e);
					}
				}
			}
		} catch (e) {
			console.error('handleSignal error:', e);
		}
	}

	function addLocalStream(stream: MediaStream) {
		for (const track of stream.getTracks()) pc.addTrack(track, stream);
		setCodecPreferences(pc);
	}

	async function replaceVideoTrack(track: MediaStreamTrack): Promise<void> {
		const sender = pc.getSenders().find(s => s.track?.kind === 'video');
		if (sender) await sender.replaceTrack(track);
	}

	async function replaceAudioTrack(track: MediaStreamTrack): Promise<void> {
		const sender = pc.getSenders().find(s => s.track?.kind === 'audio');
		if (sender) await sender.replaceTrack(track);
	}

	function restartNegotiation() {
		makeOffer();
	}

	let videoActive = true;

	/** When video is disabled, lower jitter buffer and boost audio bitrate. */
	function setVideoEnabled(enabled: boolean) {
		videoActive = enabled;
		for (const sender of pc.getSenders()) {
			const p = sender.getParameters();
			if (!p.encodings?.length) continue;
			if (sender.track?.kind === 'audio') {
				for (const enc of p.encodings) {
					// Audio-only: higher bitrate for better quality, lower for video mode
					enc.maxBitrate = enabled ? AUDIO_MAX_BITRATE : AUDIO_ONLY_BITRATE;
				}
				sender.setParameters(p).catch(() => {});
			}
		}
		for (const receiver of pc.getReceivers()) {
			if ('jitterBufferTarget' in receiver && receiver.track?.kind === 'audio') {
				// Audio-only: ultra-low jitter buffer (20ms), with video: 50ms
				(receiver as any).jitterBufferTarget = enabled ? JITTER_BUFFER_TARGET_MS : JITTER_BUFFER_AUDIO_ONLY_MS;
			}
		}
	}

	/** Derive a 4-emoji verification code from DTLS fingerprints in SDP. */
	function getVerificationEmoji(): string {
		const emojis = ['🔒', '🛡️', '🔑', '🌟', '🎯', '💎', '🔥', '🌊', '⚡', '🎵', '🍀', '🦋', '🌈', '🚀', '🎨', '🌸'];
		try {
			const local = pc.localDescription?.sdp ?? '';
			const remote = pc.remoteDescription?.sdp ?? '';
			const localFp = local.match(/a=fingerprint:\S+ ([0-9A-Fa-f:]+)/)?.[1] ?? '';
			const remoteFp = remote.match(/a=fingerprint:\S+ ([0-9A-Fa-f:]+)/)?.[1] ?? '';
			if (!localFp || !remoteFp) return '';
			// Sort so both sides get the same order
			const combined = [localFp, remoteFp].sort().join(':');
			// Simple hash from fingerprint bytes
			let hash = 0;
			for (let i = 0; i < combined.length; i++) {
				hash = ((hash << 5) - hash + combined.charCodeAt(i)) | 0;
			}
			let result = '';
			for (let i = 0; i < VERIFICATION_EMOJI_COUNT; i++) {
				result += emojis[Math.abs((hash >> (i * 4)) & 0xF)];
			}
			return result;
		} catch { return ''; }
	}

	let statsInterval: ReturnType<typeof setInterval> | null = null;
	let prevBytesReceived = 0;
	let prevTimestamp = 0;

	function startStatsPolling() {
		if (statsInterval) return;
		statsInterval = setInterval(async () => {
			if (!options.onQualityUpdate) return;
			try {
				const stats = await pc.getStats();
				let rtt = 0;
				let packetsLost = 0;
				let packetsReceived = 0;
				let bytesReceived = 0;
				let timestamp = 0;

				stats.forEach((report: any) => {
					if (report.type === 'candidate-pair' && report.state === 'succeeded') {
						rtt = report.currentRoundTripTime ? report.currentRoundTripTime * RTT_TO_MS : 0;
					}
					if (report.type === 'inbound-rtp' && report.kind === 'video') {
						packetsLost = report.packetsLost ?? 0;
						packetsReceived = report.packetsReceived ?? 0;
						bytesReceived = report.bytesReceived ?? 0;
						timestamp = report.timestamp;
					}
				});

				const packetLoss = packetsReceived > 0
					? packetsLost / (packetsLost + packetsReceived)
					: 0;

				let bitrate = 0;
				if (prevTimestamp > 0 && timestamp > prevTimestamp) {
					const dt = (timestamp - prevTimestamp) / RTT_TO_MS;
					bitrate = ((bytesReceived - prevBytesReceived) * BITS_PER_BYTE) / dt / KBPS_DIVISOR;
				}
				prevBytesReceived = bytesReceived;
				prevTimestamp = timestamp;

				const q: QualityStats = { rtt: Math.round(rtt), packetLoss, bitrate: Math.round(bitrate) };
				options.onQualityUpdate(q);

				// Adaptive: on poor quality, reduce video bitrate to prioritize audio
				if (videoActive) {
					const level = qualityLevel(q);
					for (const sender of pc.getSenders()) {
						if (sender.track?.kind !== 'video') continue;
						const p = sender.getParameters();
						if (!p.encodings?.length) continue;
						const target = level === 'poor' ? VIDEO_BITRATE_POOR : level === 'fair' ? VIDEO_BITRATE_FAIR : VIDEO_MAX_BITRATE;
						let changed = false;
						for (const enc of p.encodings) {
							if (enc.maxBitrate !== target) { enc.maxBitrate = target; changed = true; }
						}
						if (changed) sender.setParameters(p).catch(() => {});
					}
				}
			} catch { /* stats unavailable */ }
		}, STATS_POLL_INTERVAL_MS);
	}

	function stopStatsPolling() {
		if (statsInterval) { clearInterval(statsInterval); statsInterval = null; }
	}

	async function startScreenShare(): Promise<MediaStream | null> {
		try {
			const screenStream = await navigator.mediaDevices.getDisplayMedia({
				video: { cursor: 'always' } as any,
				audio: false,
			});
			const screenTrack = screenStream.getVideoTracks()[0];
			screenTrack.contentHint = 'detail';
			await replaceVideoTrack(screenTrack);
			return screenStream;
		} catch { return null; }
	}

	async function stopScreenShare(cameraTrack: MediaStreamTrack): Promise<void> {
		await replaceVideoTrack(cameraTrack);
	}

	function close() { stopStatsPolling(); pc.close(); }

	return { handleSignal, addLocalStream, replaceVideoTrack, replaceAudioTrack, restartNegotiation, setVideoEnabled, startScreenShare, stopScreenShare, startStatsPolling, stopStatsPolling, getVerificationEmoji, close };
}
