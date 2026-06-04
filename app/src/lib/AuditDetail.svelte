<script lang="ts">
  import { mcpClient } from "./McpClient";
  import type { AuditEvent } from "./types";

  interface Props {
    eventId: string | null;
    onClose: () => void;
  }
  let { eventId, onClose }: Props = $props();

  let event = $state<AuditEvent | null>(null);
  let loading = $state(false);
  let loadedFor = $state<string | null>(null);

  // React to eventId changes; load the event when it becomes non-null.
  $effect(() => {
    const id = eventId;
    if (!id) {
      event = null;
      loadedFor = null;
      return;
    }
    if (id === loadedFor) return; // already loaded
    loadedFor = id;
    loading = true;
    event = null;
    mcpClient.getAuditEvent(id)
      .then((e) => {
        // Guard against a stale id changing mid-flight
        if (loadedFor === id) {
          event = e;
          loading = false;
        }
      })
      .catch(() => {
        if (loadedFor === id) {
          event = null;
          loading = false;
        }
      });
  });
</script>

{#if eventId}
  <aside class="audit-detail" aria-label="Audit event detail" data-testid="audit-detail">
    <header>
      <h2>Audit event</h2>
      <button
        class="close"
        aria-label="Close"
        data-testid="close"
        onclick={onClose}
      >×</button>
    </header>
    {#if loading}
      <p data-testid="loading">loading…</p>
    {:else if event}
      <pre data-testid="event">{JSON.stringify(event, null, 2)}</pre>
    {:else}
      <p data-testid="not-found">event not found</p>
    {/if}
  </aside>
{/if}

<style>
  .audit-detail {
    position: fixed;
    top: 0;
    right: 0;
    width: 480px;
    height: 100vh;
    background: #1a1a1a;
    border-left: 1px solid #2a2a2a;
    padding: 1rem;
    overflow-y: auto;
    z-index: 100;
  }
  header { display: flex; justify-content: space-between; align-items: center; }
  h2 { margin: 0; font-size: 1rem; }
  .close {
    background: none;
    border: none;
    color: #ccc;
    font-size: 1.5rem;
    cursor: pointer;
  }
  pre {
    background: #111;
    padding: 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
    overflow-x: auto;
    white-space: pre-wrap;
  }
</style>
