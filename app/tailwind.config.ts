import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        display: ["'Orbitron'", "sans-serif"],
        body: ["'Rajdhani'", "sans-serif"]
      },
      colors: {
        ink: "#08162a",
        mist: "#d8fbff",
        accent: "#06b6d4",
        mint: "#22d3ee"
      }
    }
  },
  plugins: []
} satisfies Config;
