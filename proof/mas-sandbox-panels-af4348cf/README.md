driver: gpui-agent-cli 0.4.1 + cua-driver 0.64.2 (system panels only)

# A3c — print panel, Save-as-PDF, import folder picker inside the Mac App Store sandbox

App: hexuria/buwiz-forms main af4348cfbaf95ac00aeec84b274359706a5d7ba8 (M), nsh at M's NSH_PINNED_REF 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf.
Same build scaffolding as A3b: the dev macOS host has no .app, so the same host binary (built with --features app_nsh_app_ebirforms/agent, plug override nsh-gpui-agent 3e78d303 — local uncommitted rev edit, verified in the binary) runs inside a minimal hand-made eBIRForms.app wrapper, nsh dev server outside the sandbox. Entitlements: scripts/store_gmail.py --agent output minus the two identity keys, ad hoc signed (entitlements.txt: app-sandbox, files.user-selected.read-write, network.client, network.server, print all true). Container deleted before the run.

The app itself was driven ONLY through gpui-agent. cua-driver 0.64.2 touched ONLY the three system panels (print, save, open), per the job. Import itself was NEVER pressed. Queue never pressed.

Results: see report.md. Video: a3c-run.mkv — SEE report.md section "video" for the capture-gap disclosure.
