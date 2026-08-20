import {
  BIR_FOLIO,
  BIR_LEGAL,
  BIR_LETTER,
  FORM_SPEC_REGISTRY,
  getFormSpec,
  hasFormSpec,
  listFormSpecs
} from "@ebirforms/form-specs";
import { describe, expect, it } from "vitest";
import {
  FORM_COMPONENT_REGISTRY,
  getFormComponent,
  listFormComponentKeys
} from "../src/forms/registry";

const EXPECTED_FORM_KEYS = [
  "0605:1999",
  "0619E:2018",
  "0619F:2018",
  "1601C:2018",
  "1701Q:2018",
  "1701:2018",
  "1702RT:2018C",
  "1702MX:2018C",
  "2550Q:2024",
  "2551Q:2018"
];

describe("HTML form registries", () => {
  it("registers the same exact form revisions for layout and rendering", () => {
    expect(Object.keys(FORM_SPEC_REGISTRY)).toEqual(EXPECTED_FORM_KEYS);
    expect(listFormComponentKeys()).toEqual(EXPECTED_FORM_KEYS);
    expect(Object.keys(FORM_COMPONENT_REGISTRY)).toEqual(EXPECTED_FORM_KEYS);
    expect(listFormSpecs()).toHaveLength(EXPECTED_FORM_KEYS.length);
    expect(hasFormSpec("0605", "1999")).toBe(true);
    expect(hasFormSpec("0619E", "2018")).toBe(true);
    expect(hasFormSpec("0619F", "2018")).toBe(true);
    expect(hasFormSpec("2551Q", "2018")).toBe(true);
    expect(hasFormSpec("1601C", "2018")).toBe(true);
    expect(hasFormSpec("1701Q", "2018")).toBe(true);
    expect(hasFormSpec("1701", "2018")).toBe(true);
    expect(hasFormSpec("1702RT", "2018C")).toBe(true);
    expect(hasFormSpec("1702MX", "2018C")).toBe(true);
    expect(hasFormSpec("2550Q", "2024")).toBe(true);
  });

  it("returns the registered component and specification", () => {
    expect(getFormComponent("0605", "1999")).toBeDefined();
    expect(getFormSpec("0605", "1999").paper).toEqual(BIR_FOLIO);
    expect(getFormSpec("0605", "1999").expectedBasePageCount).toBe(2);
    expect(getFormComponent("0619E", "2018")).toBeDefined();
    expect(getFormSpec("0619E", "2018").paper).toEqual(BIR_LETTER);
    expect(getFormSpec("0619E", "2018").expectedBasePageCount).toBe(1);
    expect(getFormComponent("0619F", "2018")).toBeDefined();
    expect(getFormSpec("0619F", "2018").paper).toEqual(BIR_LETTER);
    expect(getFormSpec("0619F", "2018").expectedBasePageCount).toBe(1);
    expect(getFormComponent("2551Q", "2018")).toBeDefined();
    expect(getFormSpec("2551Q", "2018").expectedBasePageCount).toBe(2);
    expect(getFormComponent("1601C", "2018")).toBeDefined();
    expect(getFormSpec("1601C", "2018").paper).toEqual(BIR_FOLIO);
    expect(getFormComponent("1701Q", "2018")).toBeDefined();
    expect(getFormSpec("1701Q", "2018").paper).toEqual(BIR_FOLIO);
    expect(getFormSpec("1701Q", "2018").expectedBasePageCount).toBe(2);
    expect(getFormComponent("1701", "2018")).toBeDefined();
    expect(getFormSpec("1701", "2018").paper).toEqual(BIR_FOLIO);
    expect(getFormSpec("1701", "2018").expectedBasePageCount).toBe(4);
    expect(getFormComponent("1702RT", "2018C")).toBeDefined();
    expect(getFormSpec("1702RT", "2018C").paper).toEqual(BIR_FOLIO);
    expect(getFormSpec("1702RT", "2018C").expectedBasePageCount).toBe(4);
    expect(getFormComponent("1702MX", "2018C")).toBeDefined();
    expect(getFormSpec("1702MX", "2018C").paper).toEqual(BIR_FOLIO);
    expect(getFormSpec("1702MX", "2018C").expectedBasePageCount).toBe(4);
    expect(getFormComponent("2550Q", "2024")).toBeDefined();
    expect(getFormSpec("2550Q", "2024").paper).toEqual(BIR_LEGAL);
    expect(getFormSpec("2550Q", "2024").expectedBasePageCount).toBe(2);
  });

  it("fails closed for an unregistered revision", () => {
    expect(() => getFormComponent("1601C", "2019")).toThrow(
      "Unsupported HTML form 1601C revision 2019"
    );
    expect(() => getFormSpec("1601C", "2019")).toThrow(
      "No HTML form specification"
    );
  });
});
