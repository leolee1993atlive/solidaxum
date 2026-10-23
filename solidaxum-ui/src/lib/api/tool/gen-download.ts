/** 从 Content-Disposition 解析下载文件名，失败回退 fallback。 */
export function filenameFromDisposition(
  header: string | null,
  fallback: string
): string {
  if (!header) return fallback
  const star = header.match(/filename\*=(?:utf-8'')?([^;]+)/i)
  if (star) {
    try {
      return decodeURIComponent(star[1].trim().replace(/^"|"$/g, ''))
    } catch {
      /* ignore */
    }
  }
  const plain = header.match(/filename="?([^";]+)"?/i)
  if (plain) return plain[1].trim()
  return fallback
}
