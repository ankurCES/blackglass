<script lang="ts">
  import { mcpClient } from "./McpClient";

  interface Props {
    /** Called when the user clicks an event row, with the event id. */
    onAuditClick: (id: string) => void;
    /** Initial page size; defaults to 50 to match the core. */
    pageSize?: number;
  }
  let { onAuditClick, pageSize = 50 }: Props = $props();

  interface EventRow {
    id?: string;
    ts?: string;
    kind?: string;
    [k: string]: unknown;
  }

  let events = $state<EventRow[]>([]);
  let totalMatched = $state(0);
  let page = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let chainVerified = $state<boolean | null>(null);
  let chainHead = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      const resp = (await mcpClient.queryAudit({ kind: "all" }, page, pageSize)) as any;
      events = (resp?.events as EventRow[]) ?? [];
      totalMatched = resp?.total_matched ?? 0;
      chainVerified = resp?.hash_chain_verified ?? null;
      chainHead = resp?.hash_chain_head ?? null;
    } catch (e: any) {
      error = e?.message || String(e);
      events = [];
      totalMatched = 0;
    } finally {
      loading = false;
    }
  }

  function nextPage() { page += 1; load(); }
  function prevPage() { if (page > 0) { page -= 1; load(); } }
  function reload() { load(); }

  $effect(() => { load(); });
</script>

<section class="audit-log" aria-label="Audit log">
  <header>
    <h2>Audit log</h2>
    <div class="chain">
      {#if chainVerified === true}
        <span class="ok" data-testid="chain-ok">● chain verified</span>
      {:else if chainVerified === false}
        <span class="danger" data-testid="chain-bad">● chain NOT verified</span>
      {:else}
        <span class="muted">chain: unknown</span>
      {/if}
      {#if chainHead}
        <span class="muted" data-testid="chain-head">head: {chainHead.slice(0, 12)}…</span>
      {/if}
    </div>
    <button class="reload" onclick={reload} data-testid="reload">↻</button>
  </header>

  {#if loading}
    <p data-testid="loading">loading…</p>
  {:else if error}
    <p class="error" data-testid="error">{error}</p>
  {:else if events.length === 0}
    <p class="muted" data-testid="empty">no events yet</p>
  {:else}
    <ol class="rows" data-testid="rows">
      {#each events as ev (ev.id)}
        <li>
          <button
            class="row"
            data-testid="row"
            onclick={() => ev.id && onAuditClick(ev.id)}
            title="click to open in detail pane"
          >
            <span class="ts">{ev.ts ?? "?"}</span>
            <span class="kind">{ev.kind ?? "?"}</span>
            <span class="id">{ev.id ?? "?"}</span>
          </button>
        </li>
      {/each}
    </ol>
  {/if}
  <footer class="pagination" data-testid="pagination">
    <span class="muted">page {page + 1} · {totalMatched} total</span>
    <button onclick={prevPage} disabled={page === 0 || loading} data-testid="prev">prev</button>
    <button onclick={nextPage} disabled={loading || events.length < pageSize} data-testid="next">next</button>
  </footer>
</section>

<style>
  .audit-log {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #181818;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid #2a2a2a;
  }
  header h2 { margin: 0; font-size: 0.9rem; }
  .chain { display: flex; gap: 0.75rem; font-size: 0.8rem; align-items: center; }
  .ok { color: #4f4; }
  .danger { color: #f44; }
  .muted { color: #888; }
  .error { color: #f88; padding: 0.5rem 1rem; }
  .reload {
    margin-left: auto;
    background: none;
    border: 1px solid #2a2a2a;
    color: #ccc;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    cursor: pointer;
  }
  .rows {
    flex: 1;
    overflow-y: auto;
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 22ch 24ch 1fr;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid #222;
    color: #ccc;
    padding: 0.4rem 1rem;
    font: inherit;
    cursor: pointer;
  }
  .row:hover { background: #222; }
  .ts { color: #888; font-family: monospace; font-size: 0.8rem; }
  .kind { color: #6af; font-family: monospace; font-size: 0.8rem; }
  .id { color: #888; font-family: monospace; font-size: 0.8rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pagination {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    border-top: 1px solid #2a2a2a;
    font-size: 0.8rem;
  }
  .pagination button {
    background: #2a2a2a;
    color: #ccc;
    border: none;
    padding: 0.2rem 0.6rem;
    border-radius: 3px;
    cursor: pointer;
  }
  .pagination button:disabled { background: #1a1a1a; color: #555; cursor: not-allowed; }
</style>
