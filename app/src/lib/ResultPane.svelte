<script lang="ts">
  import type { McpRunResult } from "./types";

  interface Props {
    result: McpRunResult | null;
    onAuditClick: (id: string) => void;
  }
  let { result, onAuditClick }: Props = $props();
</script>

<aside class="result-pane" aria-label="Last run result">
  <h2>Result</h2>
  {#if !result}
    <p class="placeholder" data-testid="placeholder">No result yet. Click "Run" on a tool.</p>
  {:else if !result.ok}
    <p class="error" data-testid="error">{result.error || "tool failed"}</p>
  {:else}
    {#if result.stdout}
      <section>
        <h3>stdout</h3>
        <pre data-testid="stdout">{result.stdout}</pre>
      </section>
    {/if}
    {#if result.stderr}
      <section>
        <h3>stderr</h3>
        <pre data-testid="stderr">{result.stderr}</pre>
      </section>
    {/if}
    {#if result.audit_event_id}
      <p class="audit">
        audit:
        <button
          class="link"
          data-testid="audit-link"
          onclick={() => result?.audit_event_id && onAuditClick(result.audit_event_id)}
        >
          {result.audit_event_id}
        </button>
      </p>
    {/if}
  {/if}
</aside>

<style>
  .result-pane {
    width: 360px;
    border-left: 1px solid #2a2a2a;
    padding: 1rem;
    overflow-y: auto;
    background: #181818;
  }
  h2 { margin: 0 0 1rem 0; }
  h3 {
    font-size: 0.85rem;
    text-transform: uppercase;
    color: #888;
    margin: 1rem 0 0.25rem;
  }
  pre {
    background: #111;
    padding: 0.5rem;
    border-radius: 4px;
    overflow-x: auto;
    white-space: pre-wrap;
    font-size: 0.85rem;
  }
  .placeholder { color: #888; }
  .error { color: #f88; }
  .audit { color: #888; font-size: 0.85rem; }
  .link {
    background: none;
    border: none;
    color: #6af;
    cursor: pointer;
    padding: 0;
    font: inherit;
    text-decoration: underline;
  }
</style>
