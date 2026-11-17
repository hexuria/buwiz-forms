// The door end to end, through the real native module, against an EMPTY database in a temp folder.
// `just nsh-test` sets the dev environment (EBIR_TEST_ENV=1, ...) and no BIR_DATABASE_PATH: the door then makes
// a fresh scratch database for this engine, so nothing here touches the NSH app's data folder or anyone's data.
import { Door } from "ebirforms/native";
import { expect, test } from "nsh/test";
import { NOT_WRAPPED, WRAPPED, bir, doorStatus, hostFunctions, shutdown } from "./index";
import type { ScreenFormOpened, Taxpayer } from "./index";
import { DEMO, createDemoTaxpayer } from "./testing";

test("the door says how sending stands: a dry run, in a test environment", () => {
  const status = doorStatus();
  expect(status.sending).toBe("dry_run");
  expect(status.testEnvironment).toBe(true);
});

test("system_info answers from an empty scratch database in the temp folder", () => {
  const info = bir.system_info();
  expect(info.app_version).toBe("0.2.0");
  expect(info.os).toBe("macos");
  expect(info.database_path).toContain("ebirforms-door");
  expect(info.database_path).toContain("bir_data.db");
  // The scratch file is not the NSH app's data folder, and certainly not the owner's.
  expect(info.database_path).not.toContain("Group Containers");
  expect(info.database_path).not.toContain("BuwizNsh");
  expect(info.database_path).not.toContain(info.data_dir);
  expect(doorStatus().databasePath).toBe(info.database_path);
});

test("the form library lists 43 forms", () => {
  expect(bir.screen_forms_index()).toHaveLength(43);
  expect(bir.forms_catalog().length).toBeGreaterThan(0);
});

test("an empty database has no taxpayers; the demo taxpayer is created and listed", () => {
  expect(bir.list_taxpayers()).toHaveLength(0);
  const created = createDemoTaxpayer();
  expect(created.name).toBe("Juana Santos");
  expect(created.tin_display).toBe("000-123-453");
  expect(created.branches[0].tin_display).toBe(DEMO.tinDisplay);
  expect(created.branches[0].tin).toBe(DEMO.tin);
  const listed: Taxpayer[] = bir.list_taxpayers();
  expect(listed).toHaveLength(1);
  expect(listed[0].name).toBe("Juana Santos");
  // Asking again changes nothing.
  createDemoTaxpayer();
  expect(bir.list_taxpayers()).toHaveLength(1);
});

test("a 2551Q return: open, fill, validate, save", async () => {
  createDemoTaxpayer();
  const opened: ScreenFormOpened = await bir.screen_form_open(DEMO.tin, "2551Q", 2025, 1);
  expect(opened.form_code).toBe("2551Q");
  expect(opened.status).toBe("Draft");
  expect(opened.editable).toBe(true);
  expect(opened.values["frm2551Qv2018:txtTIN1"]).toBe("000");
  // The taxpayer's name and TIN come from the profile, not from the typing.
  expect((opened.draft as { taxpayer_name: string }).taxpayer_name).toBe("Juana Santos");

  // Fill: a box that does not exist is rejected by name, a real one takes the amount and its tax follows.
  const filled = bir.screen_form_fill("2551Q", opened.draft, {
    txtATCAmt1: "10,000.00",
    "not:a:box": "1",
  });
  expect(filled.values.txtATCAmt1).toBe("10,000.00");
  expect(filled.values.txtATCDue1).toBe("300.00");
  expect(Object.keys(filled.rejected)).toContain("not:a:box");

  // Validate: the host says what is missing, in schema keys the screen can mark.
  const validated = await bir.screen_form_validate("2551Q", filled.draft);
  expect(Array.isArray(validated.errors)).toBe(true);
  for (const error of validated.errors) {
    expect(typeof error.message).toBe("string");
    expect(Array.isArray(error.keys)).toBe(true);
  }

  // Save: a stored Draft, which then lists.
  const saved = bir.screen_form_save("2551Q", validated.draft);
  expect(saved.status).toBe("Draft");
  expect(saved.id).toBeGreaterThan(0);
  const rows = bir.list_returns("saved");
  expect(rows.length).toBe(1);
  const reopened = await bir.screen_form_open(DEMO.tin, "2551Q", 2025, 1);
  expect(reopened.values.txtATCAmt1).toBe("10,000.00");
});

test("a host error is a thrown Error with the host's message", () => {
  expect(() => bir.set_global_hotkey(1 as unknown as string)).toThrow("argument 1 must be a string, got a number");
  expect(() => Door.call("set_log_retention", [])).toThrow("argument 1 is missing; 0 were passed");
});

test("an async host error rejects with the host's message", async () => {
  await expect(Door.callAsync("import_archive", ["x.zip"])).rejects.toThrow(
    "import_archive takes no path; the host opens the file dialog",
  );
  await expect(bir.screen_form_open(DEMO.tin, "1601X", 2025, 1)).rejects.toThrow();
});

test("call and callAsync refuse the other kind and unknown names, clearly", async () => {
  expect(() => Door.call("import_archive", [])).toThrow("import_archive is an async host function: call it with callAsync, not call.");
  await expect(Door.callAsync("system_info", [])).rejects.toThrow("system_info is a sync host function: call it with call, not callAsync.");
  expect(() => Door.call("no_such_function", [])).toThrow("There is no host function named no_such_function.");
  await expect(Door.callAsync("no_such_function", [])).rejects.toThrow("There is no host function named no_such_function.");
});

test("every wrapper is a host function of its kind, and none is both wrapped and left out", () => {
  const functions = hostFunctions();
  const kinds = new Map(functions.map((entry) => [entry.name, entry.kind]));
  expect(functions.length).toBeGreaterThan(120);
  for (const wrapped of WRAPPED) expect(kinds.get(wrapped.name)).toBe(wrapped.kind);
  for (const left of NOT_WRAPPED) expect(kinds.has(left)).toBe(true);
  expect(WRAPPED.length + NOT_WRAPPED.length).toBe(functions.length);
  expect(Object.keys(bir).sort()).toEqual(WRAPPED.map((wrapped) => wrapped.name).sort());
  for (const name of Object.keys(bir)) expect(typeof (bir as Record<string, unknown>)[name]).toBe("function");
});

test("the file dialog is NSH's: a headless run has no panel, and the import says so instead of pretending", async () => {
  // The hook reached NSH's open dialog; this platform has none, so the host function fails with that reason.
  await expect(bir.import_archive()).rejects.toThrow("The file dialog could not open");
});

test("shutdown, the quit path, is safe to call twice", () => {
  // Last in the file: after it the door is for nothing but exiting (the old host called process.exit next).
  shutdown();
  shutdown();
});
