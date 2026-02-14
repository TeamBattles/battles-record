import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [
		svelte({
			hot: false
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
			$lib: '/src/lib',
			'$app/environment': '/src/tests/mocks/app-environment.ts'
		},
		coverage: {
			provider: 'v8',
			reporter: ['text', 'html'],
			include: ['src/lib/**/*.ts'],
			exclude: ['src/lib/**/*.test.ts', 'src/tests/**']
		}
	}
});
