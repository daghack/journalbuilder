import { defineConfig } from 'vite';

// Relative asset paths, so the built page works from any directory (the
// Docker image serves it at /flow/).
export default defineConfig({
  base: './',
  server: { port: 5180, strictPort: true },
  preview: { port: 5180 },
  build: { target: 'es2022' },
});
