import { getContext, setContext } from 'svelte'

class SearchState {
  open = $state(false)

  constructor() {}

  setOpen = (value: boolean) => {
    this.open = value
  }

  toggle = () => {
    this.open = !this.open
  }

  handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      this.toggle()
    }
  }
}

const SEARCH_CONTEXT_KEY = Symbol.for('scn-search')

export function setSearchContext() {
  const searchState = new SearchState()
  return setContext(SEARCH_CONTEXT_KEY, searchState)
}

export function useSearch() {
  const context = getContext<SearchState>(SEARCH_CONTEXT_KEY)
  if (!context) {
    throw new Error('useSearch must be used within a SearchProvider')
  }
  return context
}
