import { describe, expect, it } from 'vitest'
import { authUserBatchQuery, SUPER_ADMIN_ROLE_ID } from './role.js'

describe('authUserBatchQuery', () => {
  it('拼出后端 AuthUserBatchQuery 期望的逗号分隔 userIds', () => {
    expect(authUserBatchQuery('3', ['11', '12', '13'])).toBe('roleId=3&userIds=11%2C12%2C13')
  })

  it('单个 id 也走同一格式', () => {
    expect(authUserBatchQuery('3', ['11'])).toBe('roleId=3&userIds=11')
  })

  it('空数组时 userIds 为空串(后端 parse_user_ids 容忍空)', () => {
    expect(authUserBatchQuery('3', [])).toBe('roleId=3&userIds=')
  })
})

describe('SUPER_ADMIN_ROLE_ID', () => {
  it('与后端 role_service.rs 的 SUPER_ADMIN_ROLE_ID 同值', () => {
    expect(SUPER_ADMIN_ROLE_ID).toBe('1761300000000000001')
  })
})
