<script lang="ts">
  import { toolsForDomain } from "./toolCatalog";
  import { mcpClient } from "./McpClient";
  import type { Domain, McpRunResult } from "./types";

  interface Props {
    domain: Domain | null;
    onRun: (result: McpRunResult) => void;
  }
  let { domain, onRun }: Props = $props();

  let running = $state<string | null>(null);
  let argsText = $state<Record<string, string>>({});
  let lastError = $state<string | null>(null);

  async function run(toolName: string) {
    if (!domain) return;
    const argsRaw = argsText[toolName] || "{}";
    let args: unknown;
    try {
      args = JSON.parse(argsRaw);
    } catch (e: any) {
      const result: McpRunResult = { ok: false, error: `args is not valid JSON: ${e.message}` };
      lastError = result.error || "error";
      onRun(result);
      return;
    }
    running = toolName;
    lastError = null;
    try {
      const result = await mcpClient.runTool(domain, toolName, args);
      onRun(result);
    } catch (e: any) {
      const result: McpRunResult = { ok: false, error: e.message || String(e) };
      lastError = result.error || "error";
      onRun(result);
    } finally {
      running = null;
    }
  }
</script>

{#if !domain}
  <p class="hint" data-testid="hint">Select a domain from the left rail.</p>
{:else}
  <div class="tool-runner" data-testid="tool-runner">
    <h2>{domain}</h2>
    <ul>
      {#each toolsForDomain(domain) as tool (tool.name)}
        <li data-testid="tool-{tool.name}">
          <header>
            <h3>{tool.name}</h3>
            {#if tool.destructive}
              <span class="badge destructive" data-testid="badge-{tool.name}">destructive</span>
            {/if}
          </header>
          <p>{tool.description}</p>
          <details>
            <summary>args</summary>
            <textarea
              bind:value={argsText[tool.name]}
              placeholder={tool.argsSchema}
              rows="4"
              data-testid="args-{tool.name}"
            ></textarea>
          </details>
          <button
            onclick={() => run(tool.name)}
            disabled={running !== null}
            data-testid="run-{tool.name}"
          >
            {running === tool.name ? "Running…" : "Run"}
          </button>
          {#if lastError && running === null}
            <p class="inline-error" data-testid="error">{lastError}</p>
          {/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .tool-runner { padding: 1rem; }
  h2 { margin: 0 0 1rem 0; }
  ul { list-style: none; padding: 0; }
  li {
    padding: 1rem;
    border: 1px solid #2a2a2a;
    border-radius: 4px;
    margin-bottom: 1rem;
    background: #181818;
  }
  header { display: flex; align-items: center; gap: 0.5rem; }
  h3 { margin: 0; }
  .badge { font-size: 0.7rem; padding: 0.1rem 0.4rem; border-radius: 3px; }
  .badge.destructive { background: #5a1e1e; color: #fbb; }
  textarea {
    width: 100%;
    font: monospace;
    background: #111;
    color: #ccc;
    border: 1px solid #2a2a2a;
    border-radius: 3px;
    padding: 0.25rem;
  }
  .inline-error { color: #f88; margin-top: 0.5rem; }
  .hint { color: #888; padding: 1rem; }
  button {
    background: #1e3a5f;
    color: #fff;
    border: none;
    padding: 0.4rem 0.8rem;
    border-radius: 3px;
    cursor: pointer;
    font: inherit;
  }
  button:disabled { background: #444; cursor: not-allowed; }
</style>
