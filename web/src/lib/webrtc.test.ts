import { describe, it, expect } from 'vitest';
import { qualityLevel, type QualityStats } from './webrtc';

describe('qualityLevel', () => {
	it('returns good for low rtt and no packet loss', () => {
		expect(qualityLevel({ rtt: 50, packetLoss: 0, bitrate: 1000 })).toBe('good');
	});

	it('returns good at boundary (rtt=150, loss=0.03)', () => {
		expect(qualityLevel({ rtt: 150, packetLoss: 0.03, bitrate: 800 })).toBe('good');
	});

	it('returns fair for moderate rtt', () => {
		expect(qualityLevel({ rtt: 200, packetLoss: 0, bitrate: 600 })).toBe('fair');
	});

	it('returns fair for moderate packet loss', () => {
		expect(qualityLevel({ rtt: 50, packetLoss: 0.05, bitrate: 500 })).toBe('fair');
	});

	it('returns fair at upper boundary (rtt=400, loss=0.1)', () => {
		expect(qualityLevel({ rtt: 400, packetLoss: 0.1, bitrate: 300 })).toBe('fair');
	});

	it('returns poor for high rtt', () => {
		expect(qualityLevel({ rtt: 500, packetLoss: 0, bitrate: 200 })).toBe('poor');
	});

	it('returns poor for high packet loss', () => {
		expect(qualityLevel({ rtt: 50, packetLoss: 0.15, bitrate: 400 })).toBe('poor');
	});

	it('returns poor when both rtt and loss are high', () => {
		expect(qualityLevel({ rtt: 600, packetLoss: 0.2, bitrate: 100 })).toBe('poor');
	});

	it('returns good for zero stats', () => {
		expect(qualityLevel({ rtt: 0, packetLoss: 0, bitrate: 0 })).toBe('good');
	});

	it('ignores bitrate for quality assessment', () => {
		expect(qualityLevel({ rtt: 50, packetLoss: 0, bitrate: 0 })).toBe('good');
		expect(qualityLevel({ rtt: 50, packetLoss: 0, bitrate: 5000 })).toBe('good');
	});
});
