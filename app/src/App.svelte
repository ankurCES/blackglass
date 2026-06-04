<script lang="ts">
  import { onMount } from "svelte";
  import { state, workspace, selectDomain, setLastResult, openAuditDetail, closeAuditDetail } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";
  import ConfirmModal from "./lib/ConfirmModal.svelte";
  import DomainRail from "./lib/DomainRail.svelte";
  import ToolRunner from "./lib/ToolRunner.svelte";
  import ResultPane from "./lib/ResultPane.svelte";
  import AuditDetail from "./lib/AuditDetail.svelte";

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

<div class="app">
  <header class="topbar">
    <h1>blackglass</h1>
    <div class="conn">
      {#if state.conn === "connected"}
        <span class="ok">● connected</span>
      {:else if state.conn === "connecting"}
        <span class="accent">● connecting…</span>
      {:else}
        <span class="danger">● disconnected</span>
      {/if}
    </div>
  </header>
  <div class="workspace">
    <DomainRail
      selected={workspace.selectedDomain}
      onSelect={selectDomain}
    />
    <main class="middle">
      <ToolRunner
        domain={workspace.selectedDomain}
        onRun={setLastResult}
      />
    </main>
    <ResultPane
      result={workspace.lastResult}
      onAuditClick={openAuditDetail}
    />
  </div>
</div>

{#if head}
  <ConfirmModal request={head} />
{/if}

{#if workspace.auditDetailEventId}
  <AuditDetail
    eventId={workspace.auditDetailEventId}
    onClose={closeAuditDetail}
  />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    color: #ccc;
    background: #111;
  }
  .topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid #2a2a2a;
    background: #181818;
  }
  .topbar h1 {
    margin: 0;
    font-size: 0.9rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .conn { font-size: 0.8rem; }
  .ok { color: #4f4; }
  .accent { color: #fa4; }
  .danger { color: #f44; }
  .workspace {
    display: flex;
    flex: 1;
    min-height: 0; /* allow children to overflow properly */
  }
  .middle {
    flex: 1;
    overflow-y: auto;
  }
</style>
