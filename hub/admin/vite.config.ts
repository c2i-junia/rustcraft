import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			// One page for every route: the hub serves index.html for any path under /admin/ that
			// has no file, and the app routes in the browser.
			adapter: adapter({ fallback: 'index.html' }),

			// The hub serves the build under /admin/. Paths stay absolute, since the same
			// index.html answers /admin/accounts/<id> as well as /admin/.
			paths: { base: '/admin', relative: false },

			// The meta of the page; the hub sends the same policy as a header, with
			// frame-ancestors, which a meta cannot carry. Its 'sha256-…' hashes are of the inline
			// bootstrap script, added by SvelteKit here and computed by the hub for the header.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:'],
					'connect-src': ['self'],
					'base-uri': ['none'],
					'form-action': ['self'],
					'object-src': ['none']
				}
			}
		})
	],
	build: {
		// Fonts and images stay files: the policy has no data: for fonts.
		assetsInlineLimit: 0
	}
});
