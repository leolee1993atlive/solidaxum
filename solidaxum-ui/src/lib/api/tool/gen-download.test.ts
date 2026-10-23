import { describe, it, expect } from 'vitest'
import { filenameFromDisposition } from './gen-download.js'

describe('filenameFromDisposition', () => {
  it('parses RFC5987 filename*', () => {
    expect(
      filenameFromDisposition("attachment; filename*=utf-8''ruoyi.zip", 'x.zip')
    ).toBe('ruoyi.zip')
  })
  it('parses quoted filename', () => {
    expect(
      filenameFromDisposition('attachment; filename="gen.zip"', 'x.zip')
    ).toBe('gen.zip')
  })
  it('falls back when header missing', () => {
    expect(filenameFromDisposition(null, 'fallback.zip')).toBe('fallback.zip')
  })
})
