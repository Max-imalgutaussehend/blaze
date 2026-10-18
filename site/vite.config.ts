import path from 'node:path'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

const page = (p: string) => path.resolve(__dirname, p)

// Multi-page build: one HTML entry per tool, served under /blaze/ on GitHub Pages.
export default defineConfig({
  base: '/blaze/',
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  build: {
    rollupOptions: {
      input: {
        main: page('index.html'),
        ember: page('ember/index.html'),
        forge: page('forge/index.html'),
        spark: page('spark/index.html'),
        ward: page('ward/index.html'),
      },
    },
  },
})
