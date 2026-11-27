// A test fixture, not a screen: the migration pages (screens/auth/migration.tsx) for migration.test.tsx and
// shots/migration.test.tsx. `open(entry, { host: { params: { page } } })` picks the page:
//   moving | transient | transient-copy | moved | offered   (transient-copy: the copy-saved line after step 3 passed)
import { host, mount } from "nsh";
import "./theme/fonts";
import { MovedNotOpenedPage, MovingPage, OfferPage, TransientPage } from "./screens/auth/migration";
import { View } from "nsh";

const params = (host.params ?? {}) as { page?: string };
const page = params.page ?? "moving";

function Demo() {
  return (
    <View class="min-h-full w-full bg-background">
      {page === "moving" ? <MovingPage /> : null}
      {page === "transient" ? <TransientPage copySaved={false} detail="Could not reach the device folder." /> : null}
      {page === "transient-copy" ? <TransientPage copySaved detail="The move was interrupted." /> : null}
      {page === "moved" ? <MovedNotOpenedPage detail="The database did not pass its check." /> : null}
      {page === "offered" ? <OfferPage /> : null}
    </View>
  );
}

mount(Demo);
