import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'node:path'

// SoundLink 使用自定义域名:
// https://soundlink.top/
// 因此资源路径必须从根目录开始
//
// 如果未来重新部署到:
// https://KversKv.github.io/SoundLink/
// 则改回:
// base: '/SoundLink/'
//
export default defineConfig({

  base: '/',

  plugins: [
    react(),
    tailwindcss(),
  ],

  build: {

    rollupOptions: {

      input: {
        // 中文主页
        main: resolve(__dirname, 'index.html'),

        // 英文主页
        en: resolve(__dirname, 'en/index.html'),

        // 中文使用指南
        guide: resolve(__dirname, 'guide/index.html'),

        // 英文使用指南
        enGuide: resolve(__dirname, 'en/guide/index.html'),
      },

    },

  },

})