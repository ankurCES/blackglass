<script lang="ts">
  import { onMount } from "svelte";
  import { state } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";

  let unsubscribe: (() => void) | undefined;

  onMount(() => {
    state.conn = "connecting";
    listenForOperatorEvents((_e) => {
      // Task 14 wires this to the modal queue.
    }).then((un) => {
      state.conn = "connected";
      unsubscribe = un;
    }).catch(() => {
      state.conn = "disconnected";
    });
    return () => unsubscribe?.();
  });
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
    <p>Waiting for confirmation requests. (Modal lands in Task 14.)</p>
  </section>
</main>
