import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The Node backend (../server.ts) owns everything under /api — this only builds/serves the UI.
// In dev, proxy to it so one `vite` process gives you HMR against the real, running harness.
const BACKEND = `http://127.0.0.1:${process.env.UI_PORT ?? 4173}`;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": { target: BACKEND, changeOrigin: true },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
