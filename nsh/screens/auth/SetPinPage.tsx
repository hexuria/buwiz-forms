// Set this device's owner PIN, as a component. The page only collects and compares the two entries; storing is the
// injected interface's decision.
import { signal, View } from "nsh";
import { PinCells } from "../../ui";
import { ErrorLine, Labelled, OpenPage, PageButton, PageHeading } from "./parts";
import type { LockInterface } from "./lockInterface";

export function SetPinPage(props: { face: LockInterface }) {
  const first = signal("");
  const again = signal("");
  const mismatch = signal(false);
  const complete = () => first().length === 4 && again().length === 4 && first() === again();
  const compare = (value: string) => {
    again.set(value);
    if (value.length === 4 && value !== first()) {
      mismatch.set(true);
      first.set("");
      again.set("");
    } else {
      mismatch.set(false);
    }
  };
  return (
    <OpenPage testID="set-pin-page">
      <PageHeading testID="set-pin-heading">Set a PIN for this device</PageHeading>
      <Labelled label="PIN">
        <PinCells testID="set-pin-first" label="PIN" length={4} masked value={first()} onChange={(v) => { first.set(v); mismatch.set(false); }} />
      </Labelled>
      <Labelled label="Enter it again">
        <PinCells testID="set-pin-again" label="Enter it again" length={4} masked value={again()} onChange={compare} error={mismatch()} />
      </Labelled>
      <If when={mismatch()}>
        <ErrorLine message="The PINs don't match. Try again." testID="set-pin-mismatch" />
      </If>
      <View style={{ opacity: complete() ? 1 : 0.5 }}>
        <PageButton testID="set-pin-done" label="Done" onPress={() => { if (complete()) void props.face.set(first()); }} />
      </View>
    </OpenPage>
  );
}
