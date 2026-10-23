/** 个人中心的四个 tab,顺序即渲染顺序。 */
export const PROFILE_TABS = [
  'userinfo',
  'resetPwd',
  'thirdParty',
  'onlineDevice',
] as const

export type ProfileTab = (typeof PROFILE_TABS)[number]

/** 解析 URL 的 ?tab= 参数;缺省或非法一律回落 userinfo。 */
export function resolveProfileTab(raw: string | null): ProfileTab {
  return PROFILE_TABS.includes(raw as ProfileTab)
    ? (raw as ProfileTab)
    : 'userinfo'
}
