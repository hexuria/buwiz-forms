// gpui-agent proof of #528: the AuthDemo fixture's two fake account rooms, hard-coded so `nsh run` needs no test host params.
import { host } from "nsh";

Object.assign(host.params, {
  demo: true,
  second: true,
  years: true,
  mode: "solo",
  firstRun: false,
  gate: true,
  signOut: true,
  rooms: true,
  readout: true,
});
