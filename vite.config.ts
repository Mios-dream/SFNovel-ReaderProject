import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue({})],
  server: {
    port: 5173,
    watch: {
      // The controlled SF login browser keeps its own locked profile files here.
      ignored: ["**/.sfacg-login-profile/**"],
    },
    proxy: {
      "/api": "http://127.0.0.1:8787",
      "/library": "http://127.0.0.1:8787",
    },
  },
});
