// A test fixture, not a screen: two SearchPicks as the taxpayer screens use them (an RDO list of 138 and a ZIP list of 400, numbers
// only), each printing what it reported ("rdo: 050 | picks: 1 | opens: 1"), and the phone sheet's frames as its list scrolls
// (watchSheetFrames), the first and the latest, so pick.test.tsx can read that the search field stays inside the sheet.
// host.params: { rdo?: string (the starting value), required?: boolean (the RDO not clearable) }.
import { host, mount, signal, View } from "nsh";
import "../../../theme/fonts";
import { Text } from "../../../ui";
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
const said = (f: SheetFrames) => `search=${box(f.search)} list=${box(f.list)} panel=${box(f.panel)} window=${Math.round(f.window)}`;

function Probe() {
  const p = (host.params ?? {}) as { rdo?: string; required?: boolean };
  const rdo = signal(p.rdo ?? "");
  const zip = signal("");
  const counts = signal({ rdoPicks: 0, rdoOpens: 0, zipPicks: 0, zipOpens: 0 });
  const bump = (k: keyof ReturnType<typeof counts>) => counts.set({ ...counts(), [k]: counts()[k] + 1 });
  const first = signal("");
  const last = signal("");
  const reports = signal(0);
  watchSheetFrames((f) => {
    if (!first()) first.set(said(f));
    last.set(said(f));
    reports.set(reports() + 1);
  });
  return (
    <View class="flex-1 gap-4 bg-card p-6">
      <Text variant="title">Pick probe</Text>
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
      <Text>{`rdo: ${rdo()} | picks: ${counts().rdoPicks} | opens: ${counts().rdoOpens}`}</Text>
      <Text>{`zip: ${zip()} | picks: ${counts().zipPicks} | opens: ${counts().zipOpens}`}</Text>
      <Text>{`frames first: ${first()}`}</Text>
      <Text>{`frames last: ${last()}`}</Text>
      <Text>{`frames reports: ${reports()}`}</Text>
    </View>
  );
}

mount(Probe);
