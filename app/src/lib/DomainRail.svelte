<script lang="ts">
  import { DOMAINS } from "./toolCatalog";
  import type { Domain } from "./types";

  interface Props {
    selected: Domain | null;
    onSelect: (d: Domain) => void;
  }
  let { selected, onSelect }: Props = $props();

  function select(d: Domain) {
    onSelect(d);
  }
</script>

<nav class="domain-rail" aria-label="MCP domains">
  <h2>Domains</h2>
  <ul>
    {#each DOMAINS as d}
      <li>
        <button
          class:active={selected === d}
          onclick={() => select(d)}
          aria-current={selected === d ? "page" : undefined}
          data-testid="domain-{d}"
        >
          {d}
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .domain-rail {
    width: 180px;
    border-right: 1px solid #2a2a2a;
    padding: 1rem 0.5rem;
    background: #181818;
  }
  h2 {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #888;
    margin: 0 0 0.5rem 0.5rem;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: #ccc;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font: inherit;
  }
  button:hover { background: #2a2a2a; }
  button.active { background: #1e3a5f; color: #fff; }
</style>
