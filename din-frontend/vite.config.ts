import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// din-node runs by default on 127.0.0.1:8080 with permissive CORS,
// so the frontend talks to it directly (URL is configurable in the UI).
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
  },
})
