// The app's icons: the Lucide SVGs in assets/icons (copied from app/icons), imported so the bundler ships each one.
// Generated from the folder's listing; add an icon by dropping its SVG there and adding its line here.
import type { ImageAsset } from "nsh";
import i_archive from "../assets/icons/archive.svg";
import i_arrow_left from "../assets/icons/arrow-left.svg";
import i_arrow_right from "../assets/icons/arrow-right.svg";
import i_bagong_pilipinas from "../assets/icons/bagong-pilipinas.svg";
import i_bell from "../assets/icons/bell.svg";
import i_building from "../assets/icons/building.svg";
import i_calendar from "../assets/icons/calendar.svg";
import i_check from "../assets/icons/check.svg";
import i_chevron_down from "../assets/icons/chevron-down.svg";
import i_chevron_left from "../assets/icons/chevron-left.svg";
import i_chevron_right from "../assets/icons/chevron-right.svg";
import i_chevrons_down_up from "../assets/icons/chevrons-down-up.svg";
import i_chevrons_up_down from "../assets/icons/chevrons-up-down.svg";
import i_circle_check from "../assets/icons/circle-check.svg";
import i_copy from "../assets/icons/copy.svg";
import i_e_logo from "../assets/icons/e_logo.svg";
import i_eye_off from "../assets/icons/eye-off.svg";
import i_eye from "../assets/icons/eye.svg";
import i_file_text from "../assets/icons/file-text.svg";
import i_git_branch from "../assets/icons/git-branch.svg";
import i_hard_drive from "../assets/icons/hard-drive.svg";
import i_hash from "../assets/icons/hash.svg";
import i_history from "../assets/icons/history.svg";
import i_house from "../assets/icons/house.svg";
import i_info from "../assets/icons/info.svg";
import i_key_round from "../assets/icons/key-round.svg";
import i_laptop from "../assets/icons/laptop.svg";
import i_layout_template from "../assets/icons/layout-template.svg";
import i_list from "../assets/icons/list.svg";
import i_loader from "../assets/icons/loader.svg";
import i_lock from "../assets/icons/lock.svg";
import i_log_out from "../assets/icons/log-out.svg";
import i_map_pin from "../assets/icons/map-pin.svg";
import i_menu from "../assets/icons/menu.svg";
import i_minus from "../assets/icons/minus.svg";
import i_monitor from "../assets/icons/monitor.svg";
import i_moon from "../assets/icons/moon.svg";
import i_panel_left from "../assets/icons/panel-left.svg";
import i_panel_right from "../assets/icons/panel-right.svg";
import i_pencil from "../assets/icons/pencil.svg";
import i_plus from "../assets/icons/plus.svg";
import i_power from "../assets/icons/power.svg";
import i_printer from "../assets/icons/printer.svg";
import i_save from "../assets/icons/save.svg";
import i_search from "../assets/icons/search.svg";
import i_send from "../assets/icons/send.svg";
import i_settings from "../assets/icons/settings.svg";
import i_square_terminal from "../assets/icons/square-terminal.svg";
import i_square from "../assets/icons/square.svg";
import i_sun from "../assets/icons/sun.svg";
import i_trash from "../assets/icons/trash.svg";
import i_triangle_alert from "../assets/icons/triangle-alert.svg";
import i_upload from "../assets/icons/upload.svg";
import i_user from "../assets/icons/user.svg";
import i_users from "../assets/icons/users.svg";
import i_x from "../assets/icons/x.svg";

/** Every icon by its file name. */
export const ICONS = {
  archive: i_archive,
  "arrow-left": i_arrow_left,
  "arrow-right": i_arrow_right,
  "bagong-pilipinas": i_bagong_pilipinas,
  bell: i_bell,
  building: i_building,
  calendar: i_calendar,
  check: i_check,
  "chevron-down": i_chevron_down,
  "chevron-left": i_chevron_left,
  "chevron-right": i_chevron_right,
  "chevrons-down-up": i_chevrons_down_up,
  "chevrons-up-down": i_chevrons_up_down,
  "circle-check": i_circle_check,
  copy: i_copy,
  e_logo: i_e_logo,
  "eye-off": i_eye_off,
  eye: i_eye,
  "file-text": i_file_text,
  "git-branch": i_git_branch,
  "hard-drive": i_hard_drive,
  hash: i_hash,
  history: i_history,
  house: i_house,
  info: i_info,
  "key-round": i_key_round,
  laptop: i_laptop,
  "layout-template": i_layout_template,
  list: i_list,
  loader: i_loader,
  lock: i_lock,
  "log-out": i_log_out,
  "map-pin": i_map_pin,
  menu: i_menu,
  minus: i_minus,
  monitor: i_monitor,
  moon: i_moon,
  "panel-left": i_panel_left,
  "panel-right": i_panel_right,
  pencil: i_pencil,
  plus: i_plus,
  power: i_power,
  printer: i_printer,
  save: i_save,
  search: i_search,
  send: i_send,
  settings: i_settings,
  "square-terminal": i_square_terminal,
  square: i_square,
  sun: i_sun,
  trash: i_trash,
  "triangle-alert": i_triangle_alert,
  upload: i_upload,
  user: i_user,
  users: i_users,
  x: i_x,
} satisfies Record<string, ImageAsset>;

/** An icon's name: its file name in assets/icons without ".svg". */
export type IconName = keyof typeof ICONS;
