import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // 构建优化：代码分割
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          // Element Plus 单独打包
          'element-plus': ['element-plus', '@element-plus/icons-vue'],
          // Markdown 解析器
          'marked': ['marked'],
          // Vue 生态
          'vue-vendor': ['vue', 'pinia'],
        },
      },
    },
    // 提高 chunk 大小警告阈值（单位: KB）
    // Tauri 是桌面应用，不需要考虑网络加载，全量引入 Element Plus 可以接受
    chunkSizeWarningLimit: 2000,
  },
}));
