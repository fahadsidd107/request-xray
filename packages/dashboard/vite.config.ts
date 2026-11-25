import { defineConfig } from 'vite';

export default defineConfig({
  esbuild: { jsx: 'automatic' },
  server: {
    host: '127.0.0.1',
    proxy: { '/api/v1': { target: 'http://127.0.0.1:4731', changeOrigin: true } },
  },
  build: { target: 'es2022' },
});
