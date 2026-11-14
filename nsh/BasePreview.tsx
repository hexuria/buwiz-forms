// A test fixture, not a screen: the base wave's preview (the theme page and the UI kit's gallery behind a switch), as
// App.tsx showed them before the frame. screens/base/ThemePage.test.tsx and shots/base open it. It sits at the nsh root
// because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { mount, signal, View } from "nsh";
import "./theme/fonts";
import { KitPage } from "./screens/base/KitPage";
import { ThemePage } from "./screens/base/ThemePage";
import { Segmented } from "./ui";

function Preview() {
  const page = signal("theme");
  return (
    <View class="flex-1 bg-background">
      <View class="flex-row items-center justify-center border-b border-border bg-card px-4 py-2">
        <Segmented
          label="Preview"
          testID="preview-switch"
          value={page()}
          onChange={(v) => page.set(v)}
          options={[
            { value: "theme", label: "Theme" },
            { value: "kit", label: "UI kit" },
          ]}
        />
      </View>
      <If when={page() === "kit"} else={<ThemePage />}>
        <KitPage />
      </If>
    </View>
  );
}

mount(Preview);
