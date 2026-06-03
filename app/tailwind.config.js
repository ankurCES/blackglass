/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,ts}"],
  theme: {
    extend: {
      colors: {
        // Blackglass design tokens (sub-plan 3 v1; expand in v1.1).
        bg: "#0b0d10",
        surface: "#15181d",
        border: "#262a31",
        accent: "#7aa2f7",
        danger: "#f7768e",
        ok: "#9ece6a",
        muted: "#565f73",
      },
      fontFamily: {
        mono: ["ui-monospace", "SFMono-Regular", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
