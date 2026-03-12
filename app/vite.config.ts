import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// TODO: Code-split pages with React.lazy() to reduce initial bundle below 500KB
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true
  }
});
