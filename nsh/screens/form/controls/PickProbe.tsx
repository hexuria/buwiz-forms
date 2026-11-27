// A test fixture, not a screen: two SearchPicks as the taxpayer screens use them (an RDO list of 138 and a ZIP list of 400, numbers
// only), each printing what it reported ("rdo: 050 | picks: 1 | opens: 1"), and the phone sheet's frames as its list scrolls
// (watchSheetFrames), the first and the latest, so pick.test.tsx can read that the search field stays inside the sheet and that
// sheetBodyRoom() is the room the sheet's body really has.
// host.params: { rdo?: string (the starting value), required?: boolean (the RDO not clearable) }.
// A plain box under the picks ("pp-other") takes the keys when the test sends the host event "focus-other": the form's own focus()
// moving the keys with no press (a pill step, Queue's jump), so the combobox's leaving can be tested without the list hearing one.
import type { InputRef } from "nsh";
import { host, mount, signal, View } from "nsh";
import "../../../theme/fonts";
import { Text, TextField } from "../../../ui";
import type { ChoiceOption } from "../contract";
import { type FrameBox, SearchPick, type SheetFrames, watchSheetFrames } from "./Pick";

const PLACES: Record<string, string> = { "047": "East Makati", "050": "South Makati", "081": "Cebu City North", "082": "Cebu City South" };
const RDO: ChoiceOption[] = [
  { value: "", label: "Choose…" },
  ...Array.from({ length: 138 }, (_, i) => {
    const code = String(i + 1).padStart(3, "0");
    return { value: code, label: `${code} - ${PLACES[code] ?? `Revenue District ${i + 1}`}` };
  }),
];
// Three ZIPs of one city (Makati City), so a search by the city's name finds them all and no other.
const ZIP_PLACES: Record<string, string> = {
  "1209": "Bel-Air, Makati City",
  "1226": "Bel-Air, San Lorenzo, Makati City",
  "1232": "Poblacion, Makati City",
};
const ZIP: ChoiceOption[] = [
  { value: "", label: "Choose…" },
  ...Array.from({ length: 400 }, (_, i) => {
    const code = String(1000 + i);
    return { value: code, label: `${code} - ${ZIP_PLACES[code] ?? `Place ${i + 1}`}` };
  }),
];

const box = (b: FrameBox) => `${Math.round(b.top)}-${Math.round(b.bottom)}`;
/** To a tenth of a point: the sheet's own measures are compared within 1 point. */
const tenth = (n: number) => String(Math.round(n * 10) / 10);
const fine = (b: FrameBox) => `${tenth(b.top)}~${tenth(b.bottom)}`;
const said = (f: SheetFrames) =>
  `search=${box(f.search)} list=${box(f.list)} panel=${box(f.panel)} window=${Math.round(f.window)}` +
  ` header=${fine(f.header)} body=${fine(f.body)} sheet=${tenth(f.sheetTop)} room=${tenth(f.room)} max=${tenth(f.maxHeight)}`;

function Probe() {
  const p = (host.params ?? {}) as { rdo?: string; required?: boolean };
  const rdo = signal(p.rdo ?? "");
  const zip = signal("");
  const counts = signal({ rdoPicks: 0, rdoOpens: 0, zipPicks: 0, zipOpens: 0 });
  const bump = (k: keyof ReturnType<typeof counts>) => counts.set({ ...counts(), [k]: counts()[k] + 1 });
  const first = signal("");
  const last = signal("");
  const reports = signal(0);
  const other = signal("");
  let otherRef: InputRef | null = null;
  host.on("focus-other", () => otherRef?.focus());
  // The RDO pick goes away (its page, the overlay, the lock): host event "hide-rdo".
  const showRdo = signal(true);
  host.on("hide-rdo", () => showRdo.set(false));
  watchSheetFrames((f) => {
    if (!first()) first.set(said(f));
    last.set(said(f));
    reports.set(reports() + 1);
  });
  return (
    <View class="flex-1 gap-4 bg-card p-6">
      <Text variant="title">Pick probe</Text>
      <If when={showRdo()}>
        <SearchPick
          testID="pp-rdo"
          label="RDO"
          options={RDO}
          value={rdo()}
          clearable={!p.required}
          onPick={(v) => {
            rdo.set(v);
            bump("rdoPicks");
          }}
          onOpen={() => bump("rdoOpens")}
        />
      </If>
      <SearchPick
        testID="pp-zip"
        label="ZIP code"
        options={ZIP}
        value={zip()}
        searchKeyboard="digits-first"
        onPick={(v) => {
          zip.set(v);
          bump("zipPicks");
        }}
        onOpen={() => bump("zipOpens")}
      />
      <TextField
        testID="pp-other"
        accessibilityLabel="Other box"
        value={other()}
        onChange={(v) => other.set(v)}
        inputRef={(r: InputRef) => (otherRef = r)}
      />
      <Text>{`rdo: ${rdo()} | picks: ${counts().rdoPicks} | opens: ${counts().rdoOpens}`}</Text>
      <Text>{`zip: ${zip()} | picks: ${counts().zipPicks} | opens: ${counts().zipOpens}`}</Text>
      <Text>{`frames first: ${first()}`}</Text>
      <Text>{`frames last: ${last()}`}</Text>
      <Text>{`frames reports: ${reports()}`}</Text>
    </View>
  );
}

mount(Probe);
