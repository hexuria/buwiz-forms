// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) with the print preview host loaded
// (screens/print/host), for screens/print/print.test.tsx and shots/print.test.tsx. It sits at the nsh root because a real-window
// run serves assets (fonts, icons) from the entry's folder down. What a test asks for goes in `open(entry, { host: { params } })`:
//   start     the dev route ("form=2551Q/2026/3"), as ShellDemo
//   savePath  a path Save as PDF writes to without the save panel (the Rust side honours it only in a test environment)
//   pin       true: the owner (Juana Santos, "Just me") has the admin PIN 1234, so the app starts at the sign-in screen (TIN 000123453)
//             and Lock app / Log out stand in front of the frame again; the dev start route does not survive that sign-in, so
//             `Test: open the 2551Q` and `Test: open the Q2 return` open the two returns the frame shows
// Under the frame (a line the window shows, so a test reads what the preview did):
//   `preview ${code}: revision N`  while a preview of that form code is open
//   test-only buttons in the corner: `Test: switch to the second taxpayer` (Sample Trading Corp, added on first use), `Test: go Home` (the window's own layout can change once a second window opens: headless, the
//   frame reads a 0 x 0 surface then), `Test: save PDF` saves the open preview's PDF to savePath (the toolbar's own button
//   opens a panel a test cannot press), `Test: read PDF` reads that file back (pages, bytes, drawing, text).
import { host, mount, signal, Text, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO, warmForms } from "./bir/testing";
import { Print, REAL_ACTIONS, testing } from "./screens/print/native";
import { previews } from "./screens/print/store";
import "./screens/print/host";
import { app, go, store } from "./shell";
import { Shell } from "./shell/Shell";

// A start route that opens a form builds every form's schema in the frame's first render (the host does it lazily, once per process): more than the
// 2 s a headless run gives one JS call on a slow machine, where a bundle's load has 10 s.
if (/form=/.test(String((host.params as { start?: string } | null)?.start ?? ""))) warmForms();

interface Params {
  start?: string;
  savePath?: string;
  pin?: boolean;
}

/** A second taxpayer (`Test: switch to the second taxpayer` adds it and switches the frame to it). */
const OTHER_ROOT = "000111229";

const saved = signal("no pdf yet");
const read = signal("pdf not read");

function seed(pin: boolean) {
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  if (pin && !bir.auth_state().mode) {
    bir.setup_solo(taxpayer.tin_root);
    bir.set_admin_pin("1234");
  }
  if (bir.list_tax_years(head.tin).length === 0) {
    const year = new Date().getFullYear();
    bir.add_tax_year(head.tin, year, false);
    bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
  }
}

function Demo() {
  const p = host.params as Params;
  seed(p.pin === true);
  testing.savePath = p.savePath;
  const key = "2551Q";
  const line = () => {
    const now = previews.get(key);
    return now ? `preview ${key}: revision ${now.revision}` : `no preview of ${key}`;
  };
  return (
    <View class="flex-1">
      <Shell start={p.start} polling={false} />
      <View class="absolute bottom-0 left-0 gap-1 bg-card p-1">
        <Text testID="px-line">{line()}</Text>
        <Text testID="px-size">{`window ${Math.round(windowSize().width)}x${Math.round(windowSize().height)}`}</Text>
        <Text testID="px-title">{`title: ${previews.get(key)?.title ?? "none"}`}</Text>
        <Text testID="px-loads">{`page loads: ${previews.get(key)?.loads ?? 0}`}</Text>
        <Text testID="px-saved">{saved()}</Text>
        <Text testID="px-read">{read()}</Text>
        <View class="flex-row gap-2">
          <Button testID="px-home" onPress={() => go("home")}>
            <Text>Test: go Home</Text>
          </Button>
          <Button testID="px-open" onPress={() => app.openReturn({ tin: DEMO.tin, code: "2551Q", year: 2026, period: 3 })}>
            <Text>Test: open the 2551Q</Text>
          </Button>
          <Button testID="px-open-q2" onPress={() => app.openReturn({ tin: DEMO.tin, code: "2551Q", year: 2026, period: 2 })}>
            <Text>Test: open the Q2 return</Text>
          </Button>
          <Button
            testID="px-switch"
            onPress={() => {
              if (!store.taxpayers().some((t) => t.tin_root === OTHER_ROOT)) {
                store.write("create_taxpayer", { tin_root: "000-111-229", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039" } });
              }
              app.selectTaxpayer(OTHER_ROOT);
            }}
          >
            <Text>Test: switch to the second taxpayer</Text>
          </Button>
          <Button
            testID="px-save"
            onPress={() => {
              REAL_ACTIONS.savePdf(key).then(
                (path) => saved.set(`pdf saved: ${path}`),
                (error) => saved.set(`pdf failed: ${String((error as Error).message ?? error)}`),
              );
            }}
          >
            <Text>Test: save PDF</Text>
          </Button>
          <Button
            testID="px-read-pdf"
            onPress={() => {
              try {
                const probe = Print.inspectPdf(p.savePath ?? "");
                read.set(`pdf: ${probe.pages} pages, ${probe.bytes} bytes, ${probe.contentBytes} drawing, size ${Math.round(probe.widthPt)}x${Math.round(probe.heightPt)}, name ${/J\s*U\s*A\s*N\s*A/.test(probe.text) ? "found" : "missing"}, text ${probe.text.replace(/\s+/g, " ").slice(0, 80)}`);
              } catch (error) {
                read.set(`pdf unreadable: ${String((error as Error).message ?? error)}`);
              }
            }}
          >
            <Text>Test: read PDF</Text>
          </Button>
        </View>
      </View>
    </View>
  );
}

mount(Demo);
