driver: gpui-agent-cli 0.4.0

# Proof: "New tax profile" always there

Recipes in `agent-recipes/e2e/newprofile*` (suite `newprofile`), run with `just e2e <target> newprofile` at the PR head
of hexuria/buwiz-forms#492, on the iPhone simulator (phone), an iPad simulator (tablet) and the macOS desktop, light
and dark. Reports land here as `<target>-<theme>-report.md` / `-report.json` with their screenshots and recordings.
