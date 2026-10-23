import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'
// 使用 vitest/config 的 defineConfig,使 `test` 字段被类型识别 (Vitest 4 要求)。
import { defineConfig } from 'vitest/config'

export default defineConfig(({ mode }) => ({
  plugins: [tailwindcss(), sveltekit()],
  // 仅在 Vitest 测试环境下让 svelte 的 browser/client 导出优先,避免影响
  // dev/build/SSR 的模块解析。运行测试时 mode 为 'test' 且 process.env.VITEST 被设置。
  ...(mode === 'test' || process.env.VITEST
    ? {
        resolve: {
          conditions: ['browser', 'default'],
        },
      }
    : {}),
  server: {
    proxy: {
      // 后端业务 API 统一在 /api 前缀下,直接转发、不再 rewrite
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
      },
    },
  },
  preview: {
    proxy: {
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'happy-dom',
    setupFiles: ['./vitest-setup.ts'],
    include: ['src/**/*.test.ts'],
  },
}))
