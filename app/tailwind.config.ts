import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        display: ["'Space Grotesk'", "sans-serif"],
        body: ["'Source Sans 3'", "sans-serif"]
      },
      colors: {
        ink: "#102033",
        mist: "#eef4fb",
        accent: "#d95f18",
        mint: "#2b8a6e"
      }
    }
  },
  plugins: []
} satisfies Config;
