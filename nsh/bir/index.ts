// The one way into the Rust brain. Screens import from here and nowhere else:
//
//   import { bir, type Taxpayer } from "../../bir";
//   const people = bir.list_taxpayers();
//   const opened = await bir.screen_form_open(tin, "2551Q", 2025, 1);
//
// Every host function the old app calls is a typed function named as the Rust module names it; sync ones
// return the value, async ones a Promise. A host error is a thrown Error whose message is the host's.
// Boot, the send guard and the database guard happen inside the door on the first call (nsh/AGENTS.md, "The door").
export * from "./functions";
export * from "./door";
export * from "./agent";
