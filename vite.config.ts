import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tauri handles the native process; polling makes frontend hot reload
      // reliable in mounted or virtualized development environments.
      ignored: ["**/src-tauri/**"],
    },
  },
});
