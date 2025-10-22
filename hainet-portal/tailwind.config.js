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
        'hai-primary': '#007bff',
        'hai-secondary': '#6c757d',
        'hai-success': '#28a745',
        'hai-danger': '#dc3545',
        'hai-warning': '#ffc107',
        'hai-info': '#17a2b8',
        'hai-dark': '#343a40',
        'hai-light': '#f8f9fa',
      },
    },
  },
  plugins: [],
}
