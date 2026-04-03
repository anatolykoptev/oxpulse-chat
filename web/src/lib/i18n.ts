import { writable, derived, get } from 'svelte/store';

const translations = {
	ru: {
		connecting: 'Подключение',
		waiting: 'Ожидание собеседника',
		connected: 'Подключено',
		failed: 'Ошибка соединения',
		initializing: 'Инициализация',
		callEnded: 'Звонок завершён',
		closeTab: 'Можно закрыть эту вкладку',
		you: 'Вы',
		peer: 'Собеседник',
		waitingForPeer: 'Ожидание',
		share: 'Ссылка',
		copied: 'Ссылка скопирована',
		mute: 'Выключить микрофон',
		unmute: 'Включить микрофон',
		disableVideo: 'Выключить камеру',
		enableVideo: 'Включить камеру',
		hangup: 'Завершить',
		mediaError: 'Нет доступа к камере/микрофону',
		encrypted: 'Шифрование',
		verifyCode: 'Код верификации',
		heroTitle: 'Видео',
		heroTitleAccent: 'звонки',
		heroSub: 'По ссылке. Открыл — позвонил.',
		newCall: 'Новый звонок',
		joinCall: 'Присоединиться',
		roomCodePlaceholder: 'Код комнаты',
		featureE2ee: 'Чистый звук и видео',
		featureE2eeDesc: 'HD видео и чистый звук',
		featureNoSignup: 'Работает по ссылке',
		featureNoSignupDesc: 'Ничего устанавливать не нужно',
		featureRelay: 'Стабильное соединение',
		featureRelayDesc: 'Надёжное соединение',
		or: 'или',
		invalidCode: 'Неверный формат кода',
		rateCall: 'Как прошёл звонок?',
		rateThanks: 'Спасибо за отзыв!',
		redirecting: 'Переход на главную...',
		newCallBtn: 'Новый звонок',
		goHome: 'На главную',
		copyLink: 'Скопировать ссылку',
		roomCodeLabel: 'Код комнаты',
		shareScreen: 'Демонстрация экрана',
		stopShareScreen: 'Остановить демонстрацию',
		footerPrivacy: 'Конфиденциальность',
		footerTerms: 'Условия',
		footerAccessibility: 'Доступность',
		footerContact: 'Связаться',
	},
	en: {
		connecting: 'Connecting',
		waiting: 'Waiting for peer',
		connected: 'Connected',
		failed: 'Connection failed',
		initializing: 'Initializing',
		callEnded: 'Call ended',
		closeTab: 'You can close this tab',
		you: 'You',
		peer: 'Peer',
		waitingForPeer: 'Waiting',
		share: 'Share',
		copied: 'Link copied to clipboard',
		mute: 'Mute',
		unmute: 'Unmute',
		disableVideo: 'Disable video',
		enableVideo: 'Enable video',
		hangup: 'Hang up',
		mediaError: 'Camera/microphone access denied',
		encrypted: 'Encrypted',
		verifyCode: 'Verification code',
		heroTitle: 'Video',
		heroTitleAccent: 'calls',
		heroSub: 'Share a link. That\'s it.',
		newCall: 'New call',
		joinCall: 'Join',
		roomCodePlaceholder: 'Room code',
		featureE2ee: 'Clear audio & video',
		featureE2eeDesc: 'HD video and clear audio',
		featureNoSignup: 'Works by link',
		featureNoSignupDesc: 'Nothing to install',
		featureRelay: 'Stable connection',
		featureRelayDesc: 'Reliable connection',
		or: 'or',
		invalidCode: 'Invalid code format',
		rateCall: 'How was the call?',
		rateThanks: 'Thanks for your feedback!',
		redirecting: 'Redirecting to home...',
		newCallBtn: 'New call',
		goHome: 'Home',
		copyLink: 'Copy link',
		roomCodeLabel: 'Room code',
		shareScreen: 'Share screen',
		stopShareScreen: 'Stop sharing',
		footerPrivacy: 'Privacy',
		footerTerms: 'Terms',
		footerAccessibility: 'Accessibility',
		footerContact: 'Contact',
	},
} as const;

export type Locale = keyof typeof translations;
export type Translations = { [K in keyof (typeof translations)['ru']]: string };

export const LOCALES: Locale[] = ['ru', 'en'];
export const LOCALE_LABELS: Record<Locale, string> = { ru: 'RU', en: 'EN' };

function detectLocale(): Locale {
	if (typeof window === 'undefined') return 'ru';
	const urlLang = new URLSearchParams(window.location.search).get('lang');
	if (urlLang && urlLang in translations) return urlLang as Locale;
	const stored = localStorage.getItem('oxpulse-lang');
	if (stored && stored in translations) return stored as Locale;
	const browserLang = navigator.language.split('-')[0];
	if (browserLang in translations) return browserLang as Locale;
	return 'ru';
}

export const locale = writable<Locale>('ru');
export const t = derived(locale, ($locale) => translations[$locale]);

export function initLocale() {
	locale.set(detectLocale());
}

export function setLocale(l: Locale) {
	locale.set(l);
	if (typeof window !== 'undefined') {
		localStorage.setItem('oxpulse-lang', l);
		document.documentElement.lang = l;
	}
}

export function getLocale(): Locale {
	return get(locale);
}

export function getTranslations(loc?: string): Translations {
	const l = loc ?? get(locale);
	if (l in translations) return translations[l as Locale];
	return translations.ru;
}
