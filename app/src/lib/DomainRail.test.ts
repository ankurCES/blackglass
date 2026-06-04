// Tests for the DomainRail left-rail component. Asserts that:
// - one button per domain renders
// - the selected domain gets the .active class
// - clicking a domain dispatches a 'select' event with the domain id

import { describe, it, expect } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import DomainRail from "./DomainRail.svelte";
import { DOMAINS } from "./toolCatalog";

describe("DomainRail", () => {
  it("renders one button per domain", () => {
    const { getAllByRole } = render(DomainRail, { selected: null, onSelect: () => {} });
    const buttons = getAllByRole("button");
    expect(buttons.length).toBe(DOMAINS.length);
  });

  it("marks the selected domain as active", () => {
    const { getByText } = render(DomainRail, { selected: "ad", onSelect: () => {} });
    const adButton = getByText("ad");
    expect(adButton.className).toContain("active");
  });

  it("emits a select event when a domain is clicked", async () => {
    let selected: string | null = null;
    const { getByText } = render(DomainRail, {
      selected: null,
      onSelect: (d: string) => { selected = d; },
    });
    await fireEvent.click(getByText("flipper"));
    expect(selected).toBe("flipper");
  });
});
