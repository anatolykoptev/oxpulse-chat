import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: '../assets/room',
			assets: '../assets/room',
			fallback: 'index.html',
		}),
		paths: {
			base: '',
		},
	},
};

export default config;
