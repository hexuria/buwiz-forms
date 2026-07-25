# Evidence â€” 2551Q January 2018 ENCS

The exact runtime HTA, installed help, official form PDF, shared ATC catalog, and Offline eBIRForms package executable are pinned in `manifest.json`. No representative 2551Q XML was supplied. The inventory therefore fails closed to a source-derived claim: the pinned `saveXML` implementation walks `frmMain.elements` and emits every text, select-one, radio, and checkbox control while excluding buttons, hidden controls, passwords, and other element types.

Applying that predicate to the pinned DOM produces 98 static occurrences; `getRdo()` injects the RDO select into `frmMain`, producing 99 occurrences total. There are 98 distinct serialized keys because the DOM contains two `txtEmail` controls and the loop emits both under the same key. Six ATC rows are concrete and bounded. No Add/Delete row builder or unbounded family exists.

The exact ATC loader accepts every catalog payload containing `2551_`; this produces 23 entries. PT010 occurs twice with conflicting 3% and 1% rates, which is preserved and classified rather than silently deduplicated.