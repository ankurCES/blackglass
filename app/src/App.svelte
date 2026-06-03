<script lang="ts">
  import { onMount } from "svelte";
  import { state } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";
  import ConfirmModal from "./lib/ConfirmModal.svelte";

  let unsubscribe: (() => void) | undefined;

  onMount(() => {
    state.conn = "connecting";
    listenForOperatorEvents((e) => {
      if (e.kind === "confirm.request") {
        state.enqueue({
          id: e.raw.params.id,
          tool: e.raw.params.tool,
          domain: e.raw.params.domain,
          class: e.raw.params.class,
          target: e.raw.params.target,
          source: e.raw.params.source,
          deadline_in_ms: e.raw.params.deadline_in_ms,
          received_at: Date.now(),
        });
      }
    }).then((un) => {
      state.conn = "connected";
      unsubscribe = un;
    }).catch(() => {
      state.conn = "disconnected";
    });
    return () => unsubscribe?.();
  });

  let head = $derived(state.pending[0]);
</script>

<main class="h-full flex flex-col">
  <header class="border-b border-border px-4 py-2 flex items-center justify-between">
    <h1 class="text-sm tracking-wider">blackglass</h1>
    <div class="text-xs">
      {#if state.conn === "connected"}
        <span class="text-ok">● connected</span>
      {:else if state.conn === "connecting"}
        <span class="text-accent">● connecting…</span>
      {:else}
        <span class="text-danger">● disconnected</span>
      {/if}
    </div>
  </header>
  <section class="flex-1 grid place-items-center text-muted text-sm">
    {#if state.pending.length === 0}
      <p>Waiting for confirmation requests.</p>
    {:else}
      <p>{state.pending.length} pending</p>
    {/if}
  </section>
</main>

{#if head}
  <ConfirmModal request={head} />
{/if}
