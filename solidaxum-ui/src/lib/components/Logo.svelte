<script lang="ts">
  let { class: className = '', ...props } = $props()
</script>

<svg
  id="shadcn-admin-logo"
  viewBox="0 0 24 24"
  xmlns="http://www.w3.org/2000/svg"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  class="size-6 {className}"
  {...props}
>
  <title>Shadcn-Admin</title>
  <path
    d="M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"
  />
</svg>
