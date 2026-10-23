import { writable, derived, get } from 'svelte/store'

export type ConnectionStatus = 'connected' | 'disconnected' | 'checking'

interface ConnectionState {
  status: ConnectionStatus
  lastCheck: number
  retryCount: number
}

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? '/api'

// Exponential backoff for reconnect probing: 1s, 2s, 4s, 8s, ... capped.
const BACKOFF_BASE_MS = 1000
const BACKOFF_CAP_MS = 30_000

function nextBackoffMs(retryCount: number): number {
  return Math.min(BACKOFF_BASE_MS * 2 ** retryCount, BACKOFF_CAP_MS)
}

function createConnectionStore() {
  const { subscribe, update } = writable<ConnectionState>({
    status: 'connected',
    lastCheck: Date.now(),
    retryCount: 0,
  })

  let probeTimer: ReturnType<typeof setTimeout> | null = null
  let isChecking = false
  let listenersBound = false

  // ---- status transitions -------------------------------------------------

  function setStatus(status: ConnectionStatus, resetRetry: boolean): void {
    update((state) => ({
      status,
      lastCheck: Date.now(),
      retryCount: resetRetry ? 0 : state.retryCount,
    }))
  }

  // ---- reconnect probing (only runs while disconnected) -------------------

  function clearProbe(): void {
    if (probeTimer !== null) {
      clearTimeout(probeTimer)
      probeTimer = null
    }
  }

  function scheduleProbe(delayMs: number): void {
    clearProbe()
    probeTimer = setTimeout(() => {
      void checkConnection()
    }, delayMs)
  }

  /**
   * Probe the backend once. Only used to detect *recovery* while the app is
   * already showing the disconnected banner — never runs during normal use.
   */
  async function checkConnection(): Promise<boolean> {
    if (isChecking) return false
    isChecking = true

    try {
      const controller = new AbortController()
      const timeoutId = setTimeout(() => controller.abort(), 5000)

      const response = await fetch(`${API_BASE_URL}/health`, {
        method: 'GET',
        signal: controller.signal,
      })

      clearTimeout(timeoutId)

      if (
        response &&
        (response.ok || response.status === 405 || response.status === 401)
      ) {
        // 405 = Method Not Allowed (OPTIONS 不被支持但服务正常)
        // 401 = Unauthorized (服务正常但需认证)
        markConnected()
        return true
      }

      throw new Error('Connection failed')
    } catch {
      markDisconnected()
      return false
    } finally {
      isChecking = false
    }
  }

  // ---- passive signals (called by ApiClient / browser events) -------------

  function markDisconnected(): void {
    update((state) => {
      const retryCount = state.retryCount + 1
      // (Re)start backoff probe if not already scheduled.
      if (probeTimer === null) {
        scheduleProbe(nextBackoffMs(retryCount))
      }
      return {
        status: 'disconnected',
        lastCheck: Date.now(),
        retryCount,
      }
    })
  }

  function markConnected(): void {
    const wasDisconnected = get({ subscribe }).status !== 'connected'
    clearProbe()
    if (wasDisconnected) {
      setStatus('connected', true)
    }
  }

  // ---- browser event hooks ------------------------------------------------

  function handleOnline(): void {
    // Browser reports network is back — verify with the backend.
    void checkConnection()
  }

  function handleOffline(): void {
    markDisconnected()
  }

  function handleVisibilityChange(): void {
    if (document.visibilityState === 'visible') {
      const state = get({ subscribe })
      // Tab back in focus: confirm health now, but only probe if we
      // previously saw a problem (no polling when everything is fine).
      if (state.status !== 'connected') {
        void checkConnection()
      }
    }
  }

  /**
   * Bind browser-level signals. Call once from the root component (e.g.
   * ConnectionAlert onMount). Safe to call multiple times.
   */
  function init(): void {
    if (typeof window === 'undefined') return
    if (listenersBound) return
    listenersBound = true

    window.addEventListener('online', handleOnline)
    window.addEventListener('offline', handleOffline)
    document.addEventListener('visibilitychange', handleVisibilityChange)

    // If the tab is visible and we somehow start disconnected, verify now.
    if (
      document.visibilityState === 'visible' &&
      get({ subscribe }).status !== 'connected'
    ) {
      void checkConnection()
    }
  }

  /**
   * Unbind browser signals and stop any pending probe. Safe to call even if
   * init() was never called.
   */
  function destroy(): void {
    clearProbe()
    if (!listenersBound) return
    listenersBound = false

    window.removeEventListener('online', handleOnline)
    window.removeEventListener('offline', handleOffline)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  }

  return {
    subscribe,
    checkConnection,
    init,
    destroy,
    markDisconnected,
    markConnected,
  }
}

export const connectionStore = createConnectionStore()

// Derived store for easy access to connection status
export const isConnected = derived(
  connectionStore,
  ($state) => $state.status === 'connected'
)
