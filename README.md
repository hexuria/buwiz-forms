# Handoff: nsh-release

Everything a remote agent needs to move every eBIRForms download from `crates/bir-shell` to the NSH app in `nsh/`, and then delete `crates/bir-shell`.

- `GOAL.md`: the run prompt. Start here.
- `spec/PLAN.md`: the owner's picks, the facts read from the code, the two pull requests and what "done" means.
- `spec/picks.txt`: the picks as the owner gave them.

Tracking issue: hexuria/buwiz-forms#269.

This branch has no shared history with `main`. It is not a pull request and is never merged. Delete it when #269 closes: `git push origin --delete handoff/nsh-release`.
