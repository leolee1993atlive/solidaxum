import { getContext, setContext } from 'svelte'
import { getCookie, setCookie, removeCookie } from '$lib/cookies.js'

export type LayoutCollapsible = 'offcanvas' | 'icon' | 'none'
export type LayoutVariant = 'inset' | 'sidebar' | 'floating'

const LAYOUT_COLLAPSIBLE_COOKIE_NAME = 'layout_collapsible'
const LAYOUT_VARIANT_COOKIE_NAME = 'layout_variant'
const LAYOUT_COOKIE_MAX_AGE = 60 * 60 * 24 * 365 // 1 year

const VALID_COLLABSIBLE: readonly LayoutCollapsible[] = [
  'offcanvas',
  'icon',
  'none',
]
const VALID_VARIANT: readonly LayoutVariant[] = ['inset', 'sidebar', 'floating']

function isValidCollapsible(
  value: string | undefined
): value is LayoutCollapsible {
  return (
    value != null && (VALID_COLLABSIBLE as readonly string[]).includes(value)
  )
}

function isValidVariant(value: string | undefined): value is LayoutVariant {
  return value != null && (VALID_VARIANT as readonly string[]).includes(value)
}

const DEFAULT_VARIANT: LayoutVariant = 'inset'
const DEFAULT_COLLAPSIBLE: LayoutCollapsible = 'icon'

class LayoutState {
  collapsible = $state<LayoutCollapsible>(DEFAULT_COLLAPSIBLE)
  variant = $state<LayoutVariant>(DEFAULT_VARIANT)
  defaultCollapsible: LayoutCollapsible
  defaultVariant: LayoutVariant

  constructor(
    options: {
      defaultCollapsible?: LayoutCollapsible
      defaultVariant?: LayoutVariant
    } = {}
  ) {
    this.defaultCollapsible = options.defaultCollapsible || DEFAULT_COLLAPSIBLE
    this.defaultVariant = options.defaultVariant || DEFAULT_VARIANT

    if (typeof window !== 'undefined') {
      const cookieCollapsible = getCookie(LAYOUT_COLLAPSIBLE_COOKIE_NAME)
      const cookieVariant = getCookie(LAYOUT_VARIANT_COOKIE_NAME)
      this.collapsible = isValidCollapsible(cookieCollapsible)
        ? cookieCollapsible
        : this.defaultCollapsible
      this.variant = isValidVariant(cookieVariant)
        ? cookieVariant
        : this.defaultVariant
    }
  }

  setCollapsible = (collapsible: LayoutCollapsible) => {
    this.collapsible = collapsible
    setCookie(
      LAYOUT_COLLAPSIBLE_COOKIE_NAME,
      collapsible,
      LAYOUT_COOKIE_MAX_AGE
    )
  }

  setVariant = (variant: LayoutVariant) => {
    this.variant = variant
    setCookie(LAYOUT_VARIANT_COOKIE_NAME, variant, LAYOUT_COOKIE_MAX_AGE)
  }

  resetLayout = () => {
    this.collapsible = this.defaultCollapsible
    this.variant = this.defaultVariant
    removeCookie(LAYOUT_COLLAPSIBLE_COOKIE_NAME)
    removeCookie(LAYOUT_VARIANT_COOKIE_NAME)
  }
}

const LAYOUT_CONTEXT_KEY = Symbol.for('scn-layout')

export function setLayoutContext(
  options: {
    defaultCollapsible?: LayoutCollapsible
    defaultVariant?: LayoutVariant
  } = {}
) {
  const layoutState = new LayoutState(options)
  return setContext(LAYOUT_CONTEXT_KEY, layoutState)
}

export function useLayout() {
  const context = getContext<LayoutState>(LAYOUT_CONTEXT_KEY)
  if (!context) {
    throw new Error('useLayout must be used within a LayoutProvider')
  }
  return context
}
