import { describe, it, expect } from 'vitest';
import { getTranslations } from './i18n';

describe('getTranslations', () => {
	it('returns Russian translations for "ru"', () => {
		const t = getTranslations('ru');
		expect(t.connecting).toBe('Подключение');
		expect(t.waiting).toBe('Ожидание собеседника');
		expect(t.callEnded).toBe('Звонок завершён');
	});

	it('returns English translations for "en"', () => {
		const t = getTranslations('en');
		expect(t.connecting).toBe('Connecting');
		expect(t.waiting).toBe('Waiting for peer');
		expect(t.callEnded).toBe('Call ended');
	});

	it('falls back to Russian for unknown locale', () => {
		const t = getTranslations('fr');
		expect(t.connecting).toBe('Подключение');
	});

	it('falls back to Russian for empty string', () => {
		const t = getTranslations('');
		expect(t.connecting).toBe('Подключение');
	});

	it('returns all expected keys', () => {
		const t = getTranslations('ru');
		const keys = [
			'connecting', 'waiting', 'connected', 'failed', 'initializing',
			'callEnded', 'closeTab', 'you', 'peer', 'waitingForPeer',
			'share', 'copied', 'mute', 'unmute', 'disableVideo',
			'enableVideo', 'hangup', 'mediaError', 'encrypted', 'verifyCode',
		];
		for (const key of keys) {
			expect(t).toHaveProperty(key);
			expect((t as Record<string, string>)[key]).toBeTruthy();
		}
	});

	it('en and ru have the same keys', () => {
		const ru = getTranslations('ru');
		const en = getTranslations('en');
		const ruKeys = Object.keys(ru).sort();
		const enKeys = Object.keys(en).sort();
		expect(ruKeys).toEqual(enKeys);
	});
});
