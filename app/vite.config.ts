import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom'],
          'admin': [
            './src/pages/AdminDashboard.tsx',
            './src/pages/AdminUsers.tsx',
            './src/pages/AdminFleet.tsx',
            './src/pages/AdminPolicyEditor.tsx',
            './src/pages/AdminCompliance.tsx',
            './src/pages/AdminSystemHealth.tsx',
          ],
          'enterprise': [
            './src/pages/Login.tsx',
            './src/pages/Workspaces.tsx',
            './src/pages/Telemetry.tsx',
            './src/pages/UsageBilling.tsx',
            './src/pages/Integrations.tsx',
          ],
        },
      },
    },
    chunkSizeWarningLimit: 600,
  },
});
