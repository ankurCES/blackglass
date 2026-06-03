<script lang="ts">
  import { onMount } from "svelte";
  import { state, type PendingRequest } from "./state.svelte";
  import { sendResolve } from "./operator";

  let { request }: { request: PendingRequest } = $props();

  let remaining_ms = $derived(Math.max(0, request.deadline_in_ms - (state.now - request.received_at)));

  // Fire timeout on mount if the deadline already passed (shouldn't happen,
  // but defensive).
  onMount(() => {
    if (remaining_ms === 0) {
      void sendResolve(request.id, "deny");
      state.remove(request.id);
    }
  });

  // Fire timeout when the countdown hits 0 mid-display.
  $effect(() => {
    if (remaining_ms === 0) {
      void sendResolve(request.id, "deny");
      state.remove(request.id);
    }
  });

  async function decide(decision: "allow" | "allow_and_remember" | "deny") {
    await sendResolve(request.id, decision);
    state.remove(request.id);
  }

  let seconds = $derived(Math.ceil(remaining_ms / 1000));
</script>

<div class="fixed inset-0 bg-black/60 grid place-items-center z-50" data-testid="confirm-modal">
  <div class="bg-surface border border-border rounded-lg p-6 w-[480px] max-w-[90vw]">
    <header class="mb-4">
      <h2 class="text-base text-zinc-100">Operator confirmation required</h2>
      <p class="text-xs text-muted mt-1">
        <span class="text-accent">{request.tool}</span> on
        <span class="text-zinc-300">{request.target}</span>
        in domain
        <span class="text-zinc-300">{request.domain}</span>
      </p>
    </header>

    <dl class="grid grid-cols-2 gap-y-1 text-xs mb-4 font-mono">
      <dt class="text-muted">class</dt><dd class="text-danger">{request.class}</dd>
      <dt class="text-muted">source</dt><dd class="text-zinc-300">{request.source}</dd>
      <dt class="text-muted">id</dt><dd class="text-zinc-300 break-all">{request.id}</dd>
    </dl>

    <div class="text-xs text-muted mb-4">
      <!-- Sub-plan 3 v1: eta, safety_notes, etc. are stubbed (ADR Q1 = C). -->
      <p>ETA: <span class="text-zinc-300">unknown</span> · Safety: <span class="text-zinc-300">standard</span></p>
    </div>

    <footer class="flex items-center justify-between">
      <span class="text-xs" class:text-danger={remaining_ms < 5000} class:text-muted={remaining_ms >= 5000}>
        {seconds}s
      </span>
      <div class="flex gap-2">
        <button class="px-3 py-1 rounded border border-border hover:bg-bg" onclick={() => decide("deny")}>
          Deny
        </button>
        <button class="px-3 py-1 rounded border border-border hover:bg-bg" onclick={() => decide("allow_and_remember")}>
          Allow & remember
        </button>
        <button class="px-3 py-1 rounded bg-accent text-bg hover:opacity-90" onclick={() => decide("allow")}>
          Allow
        </button>
      </div>
    </footer>
  </div>
</div>
