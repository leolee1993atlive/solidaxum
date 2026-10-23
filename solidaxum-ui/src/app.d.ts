// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces

// Environment Variables
interface ImportMetaEnv {
  readonly VITE_API_BASE_URL: string
  readonly VITE_USE_MOCK_API?: string
  readonly VITE_CLIENT_ID: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

declare global {
  namespace App {
    interface Locals {
      user: { id: string; username: string } | null
    }
  }
}

declare module '@tanstack/table-core' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface ColumnMeta<TData extends RowData, TValue> {
    cellClass?: string
    align?: 'left' | 'center' | 'right'
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface TableMeta<TData extends RowData> {
    /** 后端的总数触到了计数上限:rowCount 是**下界**,真实总数 ≥ 它。
     *  分页组件据此把「共 1000 页」渲染成「共 1000+ 页」。 */
    totalCapped?: boolean
    /** 触顶时的表级估算行数(不精确、不认筛选条件),仅供展示量级。 */
    totalApprox?: number
  }
}

export {}
