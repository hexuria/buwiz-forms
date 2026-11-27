---
name: crew
description: Start or continue the launch crew as its conductor. Runs only when the user types /crew.
disable-model-invocation: true
triggers: [user]
---

1. Before anything else, run `python3 crew/bin/crew doctor` from the repository root and read what it reports. If `merge` is not `yes`, tell the user in one line that this conductor will prepare and review but not merge.
2. Take the conductor role: read `crew/roles/conductor.md`.
3. Follow `crew/playbooks/restart.md` step by step, then keep the turn loop running.
