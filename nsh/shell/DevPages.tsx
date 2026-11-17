// The two dev-only pages of the base wave, full window like the preview they came from: the theme page and the UI kit's
// gallery behind a switch. Reached from the palette ("UI kit", "Theme page"); only where the dev environment is on
// (EBIR_TEST_ENV). The kit's own tests and shots open the gallery through ../BasePreview.tsx, not through the frame.
import { View } from "nsh";
import { KitPage } from "../screens/base/KitPage";
import { ThemePage } from "../screens/base/ThemePage";
import { IconButton, Segmented } from "../ui";
import { app } from "./app";

/** The dev pages with a bar to go back to the app. */
export function DevPages() {
  return (
    <View testID="dev-pages" class="min-h-0 flex-1 bg-background">
      <View class="flex-row items-center gap-2 border-b border-border bg-card px-4 py-2">
        <IconButton testID="dev-back" icon="arrow-left" tooltip="Back to app" onPress={() => app.back()} />
        <View class="flex-1 flex-row items-center justify-center">
          <Segmented
            label="Preview"
            testID="preview-switch"
            value={app.view()}
            onChange={(v) => app.replace(v === "kit" ? "kit" : "theme")}
            options={[
              { value: "theme", label: "Theme" },
              { value: "kit", label: "UI kit" },
            ]}
          />
        </View>
      </View>
      <If when={app.view() === "kit"} else={<ThemePage />}>
        <KitPage />
      </If>
    </View>
  );
}
