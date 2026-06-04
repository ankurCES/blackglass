<script lang="ts">
  import { onMount } from "svelte";
  import { state as appState, workspace, selectDomain, setLastResult, openAuditDetail, closeAuditDetail } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";
  import ConfirmModal from "./lib/ConfirmModal.svelte";
  import DomainRail from "./lib/DomainRail.svelte";
  import ToolRunner from "./lib/ToolRunner.svelte";
  import ResultPane from "./lib/ResultPane.svelte";
  import AuditDetail from "./lib/AuditDetail.svelte";
  import AuditLog from "./lib/AuditLog.svelte";

  // Tabs in the middle pane. 'tools' shows the 3-pane workspace;
  // 'audit' shows the audit log (which also opens the AuditDetail
  // right rail on row click).
  let activeTab: "tools" | "audit" = $state("tools");

  let unsubscribe: (() => void) | undefined;

  onMount(() => {
    appState.conn = "connecting";
    listenForOperatorEvents((e) => {
      if (e.kind === "confirm.request") {
        appState.enqueue({
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
      appState.conn = "connected";
      unsubscribe = un;
    }).catch(() => {
      appState.conn = "disconnected";
    });
    return () => unsubscribe?.();
  });

  let head = $derived(appState.pending[0]);
</script>

<div class="app">
  <header class="topbar">
    <h1>blackglass</h1>
    <div class="conn">
      {#if appState.conn === "connected"}
        <span class="ok">● connected</span>
      {:else if appState.conn === "connecting"}
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
      <nav class="tabs" aria-label="Workspace views">
        <button
          class:active={activeTab === "tools"}
          onclick={() => (activeTab = "tools")}
          data-testid="tab-tools"
        >tools</button>
        <button
          class:active={activeTab === "audit"}
          onclick={() => (activeTab = "audit")}
          data-testid="tab-audit"
        >audit</button>
      </nav>
      <div class="tab-body">
        {#if activeTab === "tools"}
          <ToolRunner
            domain={workspace.selectedDomain}
            onRun={setLastResult}
          />
        {:else}
          <AuditLog onAuditClick={openAuditDetail} />
        {/if}
      </div>
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
    min-height: 0;
  }
  .middle {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .tabs {
    display: flex;
    border-bottom: 1px solid #2a2a2a;
    background: #181818;
  }
  .tabs button {
    background: none;
    border: none;
    color: #888;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font: inherit;
    text-transform: uppercase;
    font-size: 0.75rem;
    letter-spacing: 0.1em;
  }
  .tabs button:hover { color: #ccc; }
  .tabs button.active { color: #fff; border-bottom: 2px solid #1e3a5f; }
  .tab-body { flex: 1; min-height: 0; overflow-y: auto; }
</style>
