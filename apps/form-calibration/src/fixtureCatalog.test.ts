import { describe, expect, it } from "vitest";
import {
  fixtureVariantLabel,
  groupFixturesByForm,
  preferredFixture,
  type FixtureDescriptor
} from "./fixtureCatalog";

const fixtures: FixtureDescriptor[] = [
  { code: "1701", id: "1701-minimum", revision: "2018" },
  { code: "1701", id: "1701-normal", revision: "2018" },
  { code: "1701", id: "1701-long-values", revision: "2018" },
  { code: "1701Q", id: "1701q-minimum", revision: "2018" },
  { code: "1701Q", id: "1701q-normal", revision: "2018" },
  { code: "1701Q", id: "1701q-all-lines", revision: "2018" }
];

describe("calibration fixture catalog", () => {
  it("shows one form identity per exact code and revision", () => {
    const groups = groupFixturesByForm(fixtures);

    expect(groups.map((group) => group.id)).toEqual(["1701:2018", "1701Q:2018"]);
    expect(groups[0].fixtures.map((fixture) => fixture.id)).toEqual([
      "1701-minimum",
      "1701-normal",
      "1701-long-values"
    ]);
    expect(groups[1].fixtures.map((fixture) => fixture.id)).toEqual([
      "1701q-minimum",
      "1701q-normal",
      "1701q-all-lines"
    ]);
  });

  it("preserves every committed fixture with a unique descriptive label", () => {
    const labels = fixtures.map(fixtureVariantLabel);

    expect(new Set(labels).size).toBe(fixtures.length);
    expect(fixtureVariantLabel(fixtures[2])).toBe("Long Values · 1701-long-values.json");
    expect(fixtureVariantLabel(fixtures[5])).toBe("All Lines · 1701q-all-lines.json");
    expect(fixtureVariantLabel({ code: "2551Q", id: "2551q-6-rows", revision: "2018" }))
      .toBe("Canonical 6 Rows · 2551q-6-rows.json");
  });

  it("selects the normal fixture by default for a form", () => {
    expect(preferredFixture(fixtures.slice(0, 3)).id).toBe("1701-normal");
    expect(preferredFixture([fixtures[3], fixtures[5]]).id).toBe("1701q-minimum");
  });
});
