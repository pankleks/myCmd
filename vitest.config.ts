import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  resolve: { conditions: ['browser'] },
  plugins: [svelte()],
  test: {
    environment: 'node',
    css: { include: /[/\\]src[/\\]style\.css(?:\?|$)/ },
    include: ['src/**/*.test.ts', 'scripts/**/*.test.mjs'],
    coverage: {
      provider: 'v8',
      include: [
        'src/state/commander.svelte.ts',
        'src/state/controlShortcuts.ts',
        'src/operations/controller.svelte.ts',
        'src/operations/commandController.svelte.ts',
        'src/utils/markdownPreview.ts',
        'src/utils/directoryRefresh.ts',
        'src/utils/format.ts',
        'scripts/release-lib.mjs',
      ],
      reporter: ['text', 'html', 'lcov'],
      thresholds: {
        perFile: true,
        lines: 85,
        functions: 90,
        branches: 80,
        statements: 85,
      },
    },
  },
});
