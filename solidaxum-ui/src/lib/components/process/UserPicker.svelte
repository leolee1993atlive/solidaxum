<script lang="ts">
  import { userApi } from '$lib/api/system/user.js'
  import MultiSelect from '$lib/components/MultiSelect.svelte'

  let {
    value = [],
    multiple = true,
    placeholder = '请选择人员',
    onChange,
  }: {
    value?: string[]
    multiple?: boolean
    placeholder?: string
    onChange?: (next: string[]) => void
  } = $props()

  let options = $state<{ value: string; label: string }[]>([])

  $effect(() => {
    void (async () => {
      const resp = await userApi.optionselect()
      const users = (resp.data ?? resp) as Array<{ userId: string; nickName?: string; userName: string }>
      options = users.map((u) => ({
        value: String(u.userId),
        label: `${u.nickName ?? u.userName}（${u.userName}）`,
      }))
    })()
  })
</script>

<MultiSelect {options} values={value} {placeholder} onChange={(next) => onChange?.(next)} />
