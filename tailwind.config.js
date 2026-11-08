/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      fontFamily: {
        // SF Pro on macOS, then system fallbacks for Linux/Windows.
        sans: [
          '-apple-system',
          'SF Pro Text',
          'SF Pro Display',
          'Inter',
          'system-ui',
          'Segoe UI',
          'Roboto',
          'Helvetica Neue',
          'sans-serif'
        ],
        mono: [
          'SF Mono',
          'JetBrains Mono',
          'Menlo',
          'Monaco',
          'Consolas',
          'monospace'
        ]
      },
      colors: {
        // Mac-style frosted palette.
        bg: {
          base: '#f5f5f7',
          sidebar: 'rgba(245, 245, 247, 0.72)',
          card: '#ffffff',
          hover: '#ececef'
        },
        text: {
          primary: '#1d1d1f',
          secondary: '#6e6e73',
          tertiary: '#a1a1a6'
        },
        accent: {
          DEFAULT: '#007aff',
          hover: '#0066d6',
          subtle: 'rgba(0, 122, 255, 0.1)'
        },
        success: '#34c759',
        warning: '#ff9500',
        danger: '#ff3b30'
      },
      borderRadius: {
        // Soft Apple-style corners.
        xl: '12px',
        '2xl': '16px',
        '3xl': '20px'
      },
      boxShadow: {
        card: '0 1px 3px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.06)',
        popover: '0 8px 32px rgba(0,0,0,0.18)'
      },
      backdropBlur: {
        mac: '20px'
      }
    }
  },
  plugins: []
};
