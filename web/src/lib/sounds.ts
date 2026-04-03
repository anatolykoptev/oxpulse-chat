let ctx: AudioContext | null = null;

function getCtx(): AudioContext {
	if (!ctx) ctx = new AudioContext();
	if (ctx.state === 'suspended') ctx.resume().catch(() => {});
	return ctx;
}

/** Call on first user gesture (button tap) to unlock AudioContext on iOS. */
export function unlockAudio() {
	try {
		const ac = getCtx();
		if (ac.state === 'suspended') ac.resume().catch(() => {});
		// Play a silent buffer to fully unlock on iOS
		const buf = ac.createBuffer(1, 1, ac.sampleRate);
		const src = ac.createBufferSource();
		src.buffer = buf;
		src.connect(ac.destination);
		src.start();
	} catch { /* audio not available */ }
}

export function playConnectSound() {
	try {
		const ac = getCtx();
		const now = ac.currentTime;
		for (let i = 0; i < 2; i++) {
			const osc = ac.createOscillator();
			const gain = ac.createGain();
			osc.type = 'sine';
			osc.frequency.value = i === 0 ? 520 : 660;
			gain.gain.setValueAtTime(0.15, now + i * 0.15);
			gain.gain.exponentialRampToValueAtTime(0.001, now + i * 0.15 + 0.12);
			osc.connect(gain).connect(ac.destination);
			osc.start(now + i * 0.15);
			osc.stop(now + i * 0.15 + 0.12);
		}
	} catch { /* audio not available */ }
}

export function playDisconnectSound() {
	try {
		const ac = getCtx();
		const now = ac.currentTime;
		const osc = ac.createOscillator();
		const gain = ac.createGain();
		osc.type = 'sine';
		osc.frequency.setValueAtTime(480, now);
		osc.frequency.exponentialRampToValueAtTime(300, now + 0.2);
		gain.gain.setValueAtTime(0.12, now);
		gain.gain.exponentialRampToValueAtTime(0.001, now + 0.25);
		osc.connect(gain).connect(ac.destination);
		osc.start(now);
		osc.stop(now + 0.25);
	} catch { /* audio not available */ }
}

/** Gentle single-pulse ring — soft sine ping every 4 seconds. */
let ringInterval: ReturnType<typeof setInterval> | null = null;

function playRingOnce() {
	try {
		const ac = getCtx();
		const now = ac.currentTime;
		const osc = ac.createOscillator();
		const gain = ac.createGain();
		osc.type = 'sine';
		osc.frequency.value = 480;
		gain.gain.setValueAtTime(0, now);
		gain.gain.linearRampToValueAtTime(0.06, now + 0.08);
		gain.gain.exponentialRampToValueAtTime(0.001, now + 0.6);
		osc.connect(gain).connect(ac.destination);
		osc.start(now);
		osc.stop(now + 0.6);
	} catch { /* audio not available */ }
}

export function startRinging() {
	if (ringInterval) return;
	playRingOnce();
	ringInterval = setInterval(playRingOnce, 4000);
}

export function stopRinging() {
	if (ringInterval) {
		clearInterval(ringInterval);
		ringInterval = null;
	}
}
