import { describe, expect, it } from 'vitest'

import { isImageSuffix } from './oss.js'

describe('isImageSuffix', () => {
  it('识别常见图片后缀,大小写与前导点都容忍', () => {
    for (const s of ['png', '.png', 'PNG', 'jpg', 'jpeg', 'gif', 'webp', 'bmp']) {
      expect(isImageSuffix(s), `${s} 应判为图片`).toBe(true)
    }
  })

  it('非图片后缀返回 false', () => {
    for (const s of ['pdf', 'docx', 'zip', 'mp4', '', undefined]) {
      expect(isImageSuffix(s), `${s} 不应判为图片`).toBe(false)
    }
  })
})
