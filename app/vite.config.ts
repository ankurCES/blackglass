import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  resolve: process.env.VITEST
    ? { conditions: ["browser"] }
    : undefined,
  test: {
    environment: "jsdom",
    globals: true,
    server: { deps: { inline: ["@testing-library/svelte"] } },
  },
});
