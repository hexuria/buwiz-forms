// The entry. NSH mounts the root once; every later screen hangs off the frame that replaces ThemePage.
import { mount } from "nsh";
import "./theme/fonts";
import { ThemePage } from "./screens/base/ThemePage";

mount(ThemePage);
