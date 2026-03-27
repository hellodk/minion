import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  
  // Tauri expects a fixed port
  server: {
    port: 5173,
    strictPort: true,
  },
  
  // Produce sourcemaps for debugging
  build: {
    sourcemap: true,
    target: 'esnext',
  },
  
  // Env variables
  envPrefix: ['VITE_', 'TAURI_'],
});
