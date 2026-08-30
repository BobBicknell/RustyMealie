import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Tauri expects a fixed port, fail if that port is not available
  clearScreen: false,
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
      // Tell Vite to ignore watching build output directories (`src-tauri`
      // and the workspace Cargo `target/`), which otherwise balloon the
      // inotify watcher count and hit the system limit.
      ignored: ["**/src-tauri/**", "**/target/**", "**/node_modules/**"],
    },
  },
}));
