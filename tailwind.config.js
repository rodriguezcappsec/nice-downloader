/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas: "rgb(var(--canvas) / <alpha-value>)",
        panel: "rgb(var(--panel) / <alpha-value>)",
        elevated: "rgb(var(--elevated) / <alpha-value>)",
        line: "rgb(var(--line) / <alpha-value>)",
        ink: "rgb(var(--ink) / <alpha-value>)",
        muted: "rgb(var(--muted) / <alpha-value>)",
        accent: "rgb(var(--accent) / <alpha-value>)",
        accentInk: "rgb(var(--accent-ink) / <alpha-value>)",
        danger: "rgb(var(--danger) / <alpha-value>)"
      },
      boxShadow: {
        panel: "0 24px 70px rgb(17 12 5 / 0.22), inset 0 1px 0 rgb(255 248 229 / 0.035)",
        control: "0 1px 0 rgb(255 248 229 / 0.05), 0 10px 24px rgb(17 12 5 / 0.18)"
      },
      borderRadius: {
        control: "10px",
        panel: "16px",
        media: "20px"
      }
    },
  },
  plugins: [],
};
