import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: "#2563eb",
          light: "#60a5fa",
          dark: "#1d4ed8"
        }
      }
    }
  },
  plugins: []
} satisfies Config;
