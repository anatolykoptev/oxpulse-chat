import { CallSignaling, fetchTurnCredentials, type SignalMessage } from './signaling';
import { createCall, qualityLevel, type CallHandle, type QualityStats, type QualityLevel } from './webrtc';
import { playConnectSound, playDisconnectSound, startRinging, stopRinging, unlockAudio } from './sounds';

export type Status = 'init' | 'waiting' | 'connected' | 'failed' | 'ended';

interface UseCallOptions {
	get serverUrl(): string;
	get roomId(): string;
	/** Called when referrer redirect or hangup navigation is needed. */
	onEnded: () => void;
	/** Reference to the remote video element for PiP. */
	getRemoteVideo?: () => HTMLVideoElement | null;
}

export function useCall(opts: UseCallOptions) {
	let status = $state<Status>('init');
	let localStream = $state<MediaStream | null>(null);
	let remoteStream = $state<MediaStream | null>(null);
	let audioMuted = $state(false);
	let videoEnabled = $state(true);
	let elapsed = $state(0);
	let quality = $state<QualityLevel>('good');
	let controlsVisible = $state(true);
	let facingMode = $state<'user' | 'environment'>('user');
	let canFlipCamera = $state(false);
	let speakerOn = $state(true);
	let screenSharing = $state(false);
	let verificationEmoji = $state('');

	let screenStream: MediaStream | null = null;
	let call: CallHandle | null = null;
	let signaling: CallSignaling | null = null;
	let pendingSignals: SignalMessage[] = [];
	let pendingPeerJoined = false;
	let timer: ReturnType<typeof setInterval> | null = null;
	let idleTimer: ReturnType<typeof setTimeout> | null = null;
	let wakeLock: WakeLockSentinel | null = null;

	// ── Wake Lock ───────────────────────────────────────────────

	async function acquireWakeLock() {
		try {
			if ('wakeLock' in navigator) {
				wakeLock = await navigator.wakeLock.request('screen');
				wakeLock.addEventListener('release', () => { wakeLock = null; });
			}
		} catch { /* not available */ }
	}

	async function releaseWakeLock() {
		if (wakeLock) {
			await wakeLock.release().catch(() => {});
			wakeLock = null;
		}
	}

	// ── Controls visibility ─────────────────────────────────────

	function showControls() {
		controlsVisible = true;
		if (idleTimer) clearTimeout(idleTimer);
		if (status === 'connected') {
			idleTimer = setTimeout(() => { controlsVisible = false; }, 3500);
		}
	}

	// ── Device selection: prefer local mic over iPhone Continuity ──

	async function preferLocalAudio(): Promise<MediaTrackConstraints> {
		const base: MediaTrackConstraints = {
			echoCancellation: true, noiseSuppression: true,
			autoGainControl: true, sampleRate: 48000, channelCount: 1,
		};
		try {
			const devices = await navigator.mediaDevices.enumerateDevices();
			const mics = devices.filter(d => d.kind === 'audioinput');
			const iphone = /iphone|ipad/i;
			const hasLocal = mics.some(d => !iphone.test(d.label));
			const hasIphone = mics.some(d => iphone.test(d.label));
			if (hasLocal && hasIphone) {
				const local = mics.find(d => !iphone.test(d.label) && d.deviceId !== 'default');
				if (local) return { ...base, deviceId: { exact: local.deviceId } };
			}
		} catch { /* enumerateDevices not available before permission */ }
		return base;
	}

	// ── Init: media + signaling + WebRTC ────────────────────────

	async function init() {
		// Unlock AudioContext on iOS (init is triggered by user gesture)
		unlockAudio();

		const audioConstraints = await preferLocalAudio();

		try {
			localStream = await navigator.mediaDevices.getUserMedia({
				audio: audioConstraints,
				video: { width: { ideal: 1280, max: 1280 }, height: { ideal: 720, max: 720 }, frameRate: { ideal: 30 } },
			});
		} catch {
			try {
				localStream = await navigator.mediaDevices.getUserMedia({
					audio: audioConstraints,
				});
				videoEnabled = false;
			} catch {
				status = 'failed';
				return;
			}
		}

		try {
			const devices = await navigator.mediaDevices.enumerateDevices();
			canFlipCamera = devices.filter(d => d.kind === 'videoinput').length > 1;
		} catch { /* ignore */ }

		status = 'waiting';
		startRinging();

		signaling = new CallSignaling({
			async onJoined(polite: boolean) {
				const iceServers = await fetchTurnCredentials(opts.serverUrl);
				call = createCall({
					iceServers,
					polite,
					onRemoteStream(rs: MediaStream) { remoteStream = rs; },
					onConnectionState(state: RTCPeerConnectionState) {
						if (state === 'connected') {
							status = 'connected';
							stopRinging();
							playConnectSound();
							call?.startStatsPolling();
							acquireWakeLock();
							verificationEmoji = call?.getVerificationEmoji() ?? '';
							showControls();
							if (!timer) timer = setInterval(() => { elapsed += 1; }, 1000);
						} else if (state === 'failed' || state === 'closed') {
							stopRinging();
							status = 'failed';
						}
					},
					sendSignal(msg: unknown) { signaling?.sendSignal(msg); },
					onQualityUpdate(stats: QualityStats) { quality = qualityLevel(stats); },
				});
				if (localStream) call.addLocalStream(localStream);
				if (pendingPeerJoined) { call.restartNegotiation(); pendingPeerJoined = false; }
				for (const sig of pendingSignals) call.handleSignal(sig);
				pendingSignals = [];
			},
			onSignal(payload: SignalMessage) {
				if (call) call.handleSignal(payload);
				else pendingSignals.push(payload);
			},
			onPeerJoined() {
				if (call) call.restartNegotiation();
				else pendingPeerJoined = true;
			},
			onPeerLeft() {
				playDisconnectSound();
				exitPip();
				remoteStream = null;
				status = 'waiting';
				startRinging();
				controlsVisible = true;
				if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
				if (timer) { clearInterval(timer); timer = null; elapsed = 0; }
			},
			onError() { status = 'failed'; },
		});

		signaling.connect(opts.serverUrl, opts.roomId);
	}

	// ── Toggles ─────────────────────────────────────────────────

	function toggleAudio() {
		if (!localStream) return;
		audioMuted = !audioMuted;
		for (const track of localStream.getAudioTracks()) track.enabled = !audioMuted;
	}

	function toggleVideo() {
		if (!localStream) return;
		videoEnabled = !videoEnabled;
		for (const track of localStream.getVideoTracks()) track.enabled = videoEnabled;
		call?.setVideoEnabled(videoEnabled);
	}

	async function toggleSpeaker(remoteVideo: HTMLVideoElement | null) {
		speakerOn = !speakerOn;
		if (!remoteVideo || !('setSinkId' in remoteVideo)) return;
		try {
			const devices = await navigator.mediaDevices.enumerateDevices();
			const outputs = devices.filter(d => d.kind === 'audiooutput');
			if (speakerOn) {
				const speaker = outputs.find(d => d.label.toLowerCase().includes('speaker'));
				await (remoteVideo as any).setSinkId(speaker?.deviceId ?? 'default');
			} else {
				const earpiece = outputs.find(d =>
					d.label.toLowerCase().includes('earpiece') ||
					d.label.toLowerCase().includes('handset')
				);
				await (remoteVideo as any).setSinkId(earpiece?.deviceId ?? '');
			}
		} catch { /* setSinkId not supported */ }
	}

	async function flipCamera(localVideo: HTMLVideoElement | null) {
		if (!localStream || !videoEnabled) return;
		facingMode = facingMode === 'user' ? 'environment' : 'user';
		try {
			const newStream = await navigator.mediaDevices.getUserMedia({
				video: {
					facingMode: { exact: facingMode },
					width: { ideal: 1280, max: 1280 },
					height: { ideal: 720, max: 720 },
					frameRate: { ideal: 30 },
				},
			});
			const newTrack = newStream.getVideoTracks()[0];
			await call?.replaceVideoTrack(newTrack);
			const oldTrack = localStream.getVideoTracks()[0];
			if (oldTrack) { localStream.removeTrack(oldTrack); oldTrack.stop(); }
			localStream.addTrack(newTrack);
			if (localVideo) localVideo.srcObject = localStream;
		} catch {
			facingMode = facingMode === 'user' ? 'environment' : 'user';
		}
	}

	async function toggleScreenShare() {
		if (!call || !localStream) return;
		if (screenSharing) {
			const cameraTrack = localStream.getVideoTracks()[0];
			if (cameraTrack) await call.stopScreenShare(cameraTrack);
			if (screenStream) {
				for (const track of screenStream.getTracks()) track.stop();
				screenStream = null;
			}
			screenSharing = false;
		} else {
			const stream = await call.startScreenShare();
			if (stream) {
				screenStream = stream;
				screenSharing = true;
				stream.getVideoTracks()[0]?.addEventListener('ended', () => {
					toggleScreenShare();
				});
			}
		}
	}

	// ── Share link ───────────────────────────────────────────────

	function shareLink(): Promise<void> {
		return navigator.clipboard.writeText(window.location.href);
	}

	// ── Hangup ──────────────────────────────────────────────────

	function hangup() {
		stopRinging();
		if (timer) { clearInterval(timer); timer = null; }
		if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
		releaseWakeLock();
		exitPip();
		call?.close(); call = null;
		signaling?.close(); signaling = null;
		if (screenStream) {
			for (const track of screenStream.getTracks()) track.stop();
			screenStream = null;
			screenSharing = false;
		}
		if (localStream) {
			for (const track of localStream.getTracks()) track.stop();
			localStream = null;
		}
		remoteStream = null;
		status = 'ended';
		opts.onEnded();
	}

	// ── iOS resume: restore dead tracks after app switch ────────

	async function restoreMediaTracks() {
		if (!localStream) return;

		const audioTrack = localStream.getAudioTracks()[0];
		const videoTrack = localStream.getVideoTracks()[0];
		const audioDead = !audioTrack || audioTrack.readyState === 'ended' || !audioTrack.enabled;
		const videoDead = videoEnabled && (!videoTrack || videoTrack.readyState === 'ended');

		if (!audioDead && !videoDead) return;

		try {
			const constraints: MediaStreamConstraints = {};
			if (audioDead) {
				constraints.audio = await preferLocalAudio();
			}
			if (videoDead) {
				constraints.video = {
					facingMode: facingMode,
					width: { ideal: 1280, max: 1280 },
					height: { ideal: 720, max: 720 },
					frameRate: { ideal: 30 },
				};
			}

			const freshStream = await navigator.mediaDevices.getUserMedia(constraints);

			// Replace audio track
			if (audioDead) {
				const newAudio = freshStream.getAudioTracks()[0];
				if (newAudio) {
					if (audioTrack) { localStream.removeTrack(audioTrack); audioTrack.stop(); }
					localStream.addTrack(newAudio);
					newAudio.enabled = !audioMuted;
					await call?.replaceAudioTrack(newAudio);
				}
			}

			// Replace video track
			if (videoDead) {
				const newVideo = freshStream.getVideoTracks()[0];
				if (newVideo) {
					if (videoTrack) { localStream.removeTrack(videoTrack); videoTrack.stop(); }
					localStream.addTrack(newVideo);
					await call?.replaceVideoTrack(newVideo);
				}
			}

			// Force re-assignment so $effect picks up the change for srcObject binding
			localStream = localStream;
		} catch { /* getUserMedia failed on resume — camera may be in use by another app */ }
	}

	// ── Picture-in-Picture ──────────────────────────────────────

	let pipActive = $state(false);

	function canPip(): boolean {
		return 'pictureInPictureEnabled' in document && document.pictureInPictureEnabled;
	}

	async function enterPip() {
		const video = opts.getRemoteVideo?.();
		if (!video || !canPip() || document.pictureInPictureElement) return;
		try {
			await video.requestPictureInPicture();
			pipActive = true;
		} catch { /* PiP not available */ }
	}

	function exitPip() {
		if (document.pictureInPictureElement) {
			document.exitPictureInPicture().catch(() => {});
		}
		pipActive = false;
	}

	function togglePip() {
		if (pipActive) exitPip();
		else enterPip();
	}

	// ── Cleanup ─────────────────────────────────────────────────

	function handleVisibilityChange() {
		if (document.visibilityState === 'hidden') {
			// Auto-enter PiP when backgrounding during a call
			if (status === 'connected' && remoteStream && !document.pictureInPictureElement) {
				enterPip();
			}
		} else {
			if (status === 'connected' || status === 'waiting') {
				acquireWakeLock();
				restoreMediaTracks();
			}
		}
	}

	// Listen for PiP exit (user closes PiP window)
	function handlePipExit() {
		pipActive = false;
	}

	function beforeUnload() {
		stopRinging();
		call?.close();
		signaling?.close();
		if (localStream) for (const track of localStream.getTracks()) track.stop();
		releaseWakeLock();
	}

	function destroy() {
		stopRinging();
		if (timer) clearInterval(timer);
		if (idleTimer) clearTimeout(idleTimer);
		releaseWakeLock();
		exitPip();
		call?.close();
		signaling?.close();
		if (localStream) for (const track of localStream.getTracks()) track.stop();
	}

	// ── Derived ─────────────────────────────────────────────────

	const isLocalPip = $derived(status === 'connected' && !!remoteStream);
	const isMobile = $derived(typeof window !== 'undefined' && 'ontouchstart' in window);
	const timerStr = $derived(() => {
		const m = Math.floor(elapsed / 60);
		const s = (elapsed % 60).toString().padStart(2, '0');
		return `${m}:${s}`;
	});

	return {
		// Reactive state (getters)
		get status() { return status; },
		get localStream() { return localStream; },
		get remoteStream() { return remoteStream; },
		get audioMuted() { return audioMuted; },
		get videoEnabled() { return videoEnabled; },
		get elapsed() { return elapsed; },
		get quality() { return quality; },
		get controlsVisible() { return controlsVisible; },
		get canFlipCamera() { return canFlipCamera; },
		get speakerOn() { return speakerOn; },
		get screenSharing() { return screenSharing; },
		get verificationEmoji() { return verificationEmoji; },
		get isLocalPip() { return isLocalPip; },
		get pipActive() { return pipActive; },
		get canPip() { return canPip(); },
		get isMobile() { return isMobile; },
		get timerStr() { return timerStr; },

		// Actions
		init,
		toggleAudio,
		toggleVideo,
		toggleSpeaker,
		flipCamera,
		toggleScreenShare,
		togglePip,
		shareLink,
		hangup,
		showControls,

		// Lifecycle
		handleVisibilityChange,
		handlePipExit,
		beforeUnload,
		destroy,
	};
}
