import { defineConfig } from 'vitest/config';
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [
		// SvelteKit 3 has no svelte.config.js, so the preprocessor the app uses must be passed here
		svelte({
			preprocess: vitePreprocess()
		})
	],
	resolve: {
		// Ensure browser conditions are used for Svelte
		conditions: ['browser']
	},
	test: {
		include: ['src/**/*.{test,spec}.ts'],
		environment: 'happy-dom',
		globals: true,
		setupFiles: ['./src/tests/setup.ts'],
		alias: {
			'$app/env': '/src/tests/mocks/app-env.ts'
		},
		coverage: {
			provider: 'v8',
			reporter: ['text', 'html'],
			include: ['src/lib/**/*.ts'],
			exclude: ['src/lib/**/*.test.ts', 'src/tests/**']
		}
	}
});
