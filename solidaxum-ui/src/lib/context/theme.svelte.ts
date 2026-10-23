import { getContext, setContext } from 'svelte'
import { getCookie, setCookie, removeCookie } from '$lib/cookies.js'
import {
  COLOR_THEMES,
  COLOR_THEME_COOKIE_NAME,
  DEFAULT_COLOR_THEME,
  getColorTheme,
  isValidColorTheme,
  type ColorThemeName,
} from '$lib/themes/registry.js'

export type Theme = 'dark' | 'light' | 'system'
export type ResolvedTheme = Exclude<Theme, 'system'>

const VALID_THEMES: readonly Theme[] = ['dark', 'light', 'system']

function isValidTheme(value: string | undefined): value is Theme {
  return value != null && (VALID_THEMES as readonly string[]).includes(value)
}

const DEFAULT_THEME: Theme = 'system'
const THEME_COOKIE_NAME = 'vite-ui-theme'
const THEME_COOKIE_MAX_AGE = 60 * 60 * 24 * 365 // 1 year

class ThemeState {
  theme = $state<Theme>(DEFAULT_THEME)
  storageKey: string
  defaultTheme: Theme

  resolvedTheme = $derived.by((): ResolvedTheme => {
    if (this.theme !== 'dark' && this.theme !== 'light') {
      if (typeof window !== 'undefined') {
        return window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light'
      }
      return 'light' // Default for SSR
    }
    return this.theme
  })

  colorTheme = $state<ColorThemeName>(DEFAULT_COLOR_THEME)
  resolvedColorTheme = $derived(this.colorTheme)
  colorThemeStorageKey: string

  constructor(
    options: {
      defaultTheme?: Theme
      storageKey?: string
      colorThemeStorageKey?: string
    } = {}
  ) {
    this.defaultTheme = options.defaultTheme || DEFAULT_THEME
    this.storageKey = options.storageKey || THEME_COOKIE_NAME
    this.colorThemeStorageKey =
      options.colorThemeStorageKey || COLOR_THEME_COOKIE_NAME

    if (typeof window !== 'undefined') {
      const cookieValue = getCookie(this.storageKey)
      this.theme = isValidTheme(cookieValue) ? cookieValue : this.defaultTheme

      const colorCookieValue = getCookie(this.colorThemeStorageKey)
      this.colorTheme =
        colorCookieValue != null && isValidColorTheme(colorCookieValue)
          ? (colorCookieValue as ColorThemeName)
          : DEFAULT_COLOR_THEME
    }
  }

  setTheme = (theme: Theme) => {
    this.theme = theme
    setCookie(this.storageKey, theme, THEME_COOKIE_MAX_AGE)
  }

  resetTheme = () => {
    this.theme = this.defaultTheme
    removeCookie(this.storageKey)
  }

  setColorTheme = (name: string) => {
    if (!isValidColorTheme(name)) return
    this.colorTheme = name as ColorThemeName
    setCookie(this.colorThemeStorageKey, name, THEME_COOKIE_MAX_AGE)
  }

  resetColorTheme = () => {
    this.colorTheme = DEFAULT_COLOR_THEME
    removeCookie(this.colorThemeStorageKey)
  }

  // Helper to apply theme to document element
  applyToDocument = () => {
    if (typeof window === 'undefined') return
    const root = window.document.documentElement
    root.classList.remove('light', 'dark')
    root.classList.add(this.resolvedTheme)

    for (const ct of COLOR_THEMES) {
      root.classList.remove(ct.cssClass)
    }
    const currentColorTheme = getColorTheme(this.resolvedColorTheme)
    if (currentColorTheme) {
      root.classList.add(currentColorTheme.cssClass)
    }
  }
}

const THEME_CONTEXT_KEY = Symbol.for('scn-theme')

export function setThemeContext(
  options: {
    defaultTheme?: Theme
    storageKey?: string
    colorThemeStorageKey?: string
  } = {}
) {
  const themeState = new ThemeState(options)
  return setContext(THEME_CONTEXT_KEY, themeState)
}

export function useTheme() {
  const context = getContext<ThemeState>(THEME_CONTEXT_KEY)
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return context
}
