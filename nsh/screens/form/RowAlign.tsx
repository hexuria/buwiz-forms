// A test fixture, not a screen: one multi-control row as the page draws it (rows.tsx Controls: labelled columns,
// "flex-row flex-wrap items-end"), a long choice (more than SEARCH_FROM options: the searchable pick) beside a locked
// box, as 2551Q Item 2 has "Year Ended MM" beside "Year". Each column, label and box reports its laid-out frame as a
// text ("ctlA y=21 h=32"), so rowAlign.test.tsx reads that the boxes sit on one line.
import type { Bounds } from "nsh";
import { mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import { FieldControl } from "./controls";
import { mockBox, type MockBox } from "./controls/mock";
import type { FieldInput } from "./controls/types";

const frames = signal<Record<string, string>>({});
const report = (name: string) => (e: { bounds: Bounds }) =>
  frames.set({ ...frames(), [name]: `${name} y=${e.bounds.origin.y} h=${e.bounds.size.height}` });

function input(k: string, t: FieldInput["field"]["t"], box: MockBox): FieldInput {
  return { field: { k, l: k, t }, label: k, testID: `ra-${k}`, box: box.state, width: 130, narrow: () => false, onEdit: () => {}, onFocus: () => {}, onBlur: () => {} };
}

function Fixture() {
  const months = [{ value: "", label: "Choose…" }, ...Array.from({ length: 12 }, (_, i) => ({ value: String(i + 1), label: `${i + 1} - Month` }))];
  const pick = mockBox({ value: "12", options: months });
  const year = mockBox({ value: "2026", locked: true });
  return (
    <View class="flex-1 bg-card p-10">
      <View class="flex-row flex-wrap items-end gap-x-4 gap-y-3">
        <View class="min-w-0 gap-1" onLayout={report("colA")}>
          <Text class="text-meta">Year Ended MM</Text>
          <View onLayout={report("ctlA")}>
            <FieldControl {...input("month", "sel", pick)} />
          </View>
        </View>
        <View class="min-w-0 gap-1" onLayout={report("colB")}>
          <Text class="text-meta">Year</Text>
          <View onLayout={report("ctlB")}>
            <FieldControl {...input("year", "text", year)} />
          </View>
        </View>
      </View>
      <Each of={Object.values(frames())}>{(t) => <Text>{t()}</Text>}</Each>
    </View>
  );
}

mount(Fixture);
