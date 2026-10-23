/// <reference types="@testing-library/jest-dom/vitest" />
import { describe, it, expect } from 'vitest'

describe('vitest sanity', () => {
  it('happy-dom provides document', () => {
    const el = document.createElement('div')
    el.textContent = 'hello'
    expect(el).toHaveTextContent('hello')
  })
})
