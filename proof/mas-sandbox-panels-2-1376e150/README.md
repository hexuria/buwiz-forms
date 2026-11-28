driver: gpui-agent-cli 0.4.1 + cua-driver 0.64.2 (system panels only)

# A3d — MAS sandbox gate, round 2, at main 1376e150eb964d3cae8aa957bcc970b0fb9e6dcb (M)

Same sandboxed dev build as A3c: dev host binary (agent feature, plug override nsh-gpui-agent 3e78d303 — local uncommitted rev edit, verified in the binary; the pin at M is db0e4ac which does NOT include the #14 fix) inside the same minimal hand-made eBIRForms.app wrapper (no real .app exists for the dev host), nsh dev server outside the sandbox, entitlements = scripts/store_gmail.py --agent minus the two identity keys, ad hoc signed, fresh container. App driven ONLY through gpui-agent; cua-driver 0.64.2 (both TCC grants verified on at job start) touched ONLY the print/save/open panels. Import never pressed; Queue never pressed. ~ = the home folder.

Video: a3d-run.mkv, 80 s — the screen recorder stopped writing segments after about a minute AGAIN (second time; post-processing also produced a truncated 9 s edit). It covers the print preview and the print panel opening. Per the job's fallback the panel proof is the screenshots plus the artifacts: pictures/print-preview.png, pictures/save-panel.png (the save panel mid-drive), pictures/import-scan.png (the scan result), and the PDF itself.
