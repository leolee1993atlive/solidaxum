import {
  createTable,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
} from '@tanstack/table-core'

/**
 * Get the full default state for a TanStack table.
 * Needed because createTable's getState() returns options.state directly
 * without merging with initialState. Features like columnPinning access
 * getState().columnPinning.left directly, so the full state must be present.
 */
const _t = createTable({
  data: [],
  columns: [],
  state: {},
  onStateChange: () => {},
  getCoreRowModel: getCoreRowModel(),
  getSortedRowModel: getSortedRowModel(),
  getFilteredRowModel: getFilteredRowModel(),
  getPaginationRowModel: getPaginationRowModel(),
  renderFallbackValue: null,
})

export const TABLE_DEFAULT_STATE = _t.initialState
