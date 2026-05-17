//! # START OF FILE hainet-portal/tailwind.config.js
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'theme-bg-primary': 'var(--bg-primary)',
        'theme-bg-secondary': 'var(--bg-secondary)',
        'theme-bg-tertiary': 'var(--bg-tertiary)',
        'theme-text-primary': 'var(--text-primary)',
        'theme-text-secondary': 'var(--text-secondary)',
        'theme-text-muted': 'var(--text-muted)',
        'theme-accent-primary': 'var(--accent-primary)',
        'theme-accent-secondary': 'var(--accent-secondary)',
        'theme-accent-success': 'var(--accent-success)',
        'theme-accent-danger': 'var(--accent-danger)',
        'theme-border': 'var(--border-color)',
      },
    },
  },
  plugins: [],
}
