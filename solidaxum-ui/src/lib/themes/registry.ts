export type ColorThemeName =
  | 'teal'
  | 'default'
  | 'de-swiss-design'
  | 'ibm-carbon'

export interface ColorTheme {
  name: ColorThemeName
  label: string
  cssClass: string
  cssPath: string
}

export const COLOR_THEMES: readonly ColorTheme[] = [
  {
    name: 'teal',
    label: '青绿',
    cssClass: 'theme-teal',
    cssPath: '$lib/themes/teal.css',
  },
  {
    name: 'default',
    label: '默认',
    cssClass: 'theme-default',
    cssPath: '$lib/themes/default.css',
  },
  {
    name: 'de-swiss-design',
    label: 'DE Swiss Design',
    cssClass: 'theme-de-swiss-design',
    cssPath: '$lib/themes/de-swiss-design.css',
  },
  {
    name: 'ibm-carbon',
    label: 'IBM Carbon',
    cssClass: 'theme-ibm-carbon',
    cssPath: '$lib/themes/ibm-carbon.css',
  },
]

const VALID_COLOR_THEMES: readonly string[] = COLOR_THEMES.map((t) => t.name)

export const DEFAULT_COLOR_THEME: ColorThemeName = 'teal'

export const COLOR_THEME_COOKIE_NAME = 'app-color-theme'

export function getColorTheme(name: string): ColorTheme | undefined {
  return COLOR_THEMES.find((t) => t.name === name)
}

export function isValidColorTheme(name: string): boolean {
  return VALID_COLOR_THEMES.includes(name)
}
