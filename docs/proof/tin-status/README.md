driver: gpui-agent-cli 0.5.0

# D2 proof: TIN status on screen

Pictures and a recording of the chips, the Activate flow and the Queue sheet, driven only through gpui-agent-cli
recipes (`just e2e <target> tin-status --allow-send --record`, recipes in `agent-recipes/e2e/`) against the local
mock account server (`just mock-account`, crates/bir-core/src/mock_account.rs). Nothing but the mock is contacted;
dry-run SFTP, no keychain, test env. Never a simctl tap, osascript or CGEvent: steps gpui-agent cannot do are
known_gap entries with their issue numbers.

Suite `agent-recipes/e2e/tin-status.json`: setup-start, sign-up (tin-sign-up types the 9-digit TIN root into the
segmented control), buwiz-sign-in (sign_in: true; the code goes as keys to gate-code-page while the code boxes
have focus), tin-chips, tin-queue-sheet (the sheet stops Queue, nothing is queued).

Status of this branch: the macOS run at the final head (3cf68db4dbe6, gpui-agent-cli 0.5.0
installed from crates.io) passes setup-start and sign-up end to end (screenshots in macos/), and buwiz-sign-in
drives Settings > Account > Sign in, the email step and the code send — then stops at keying the code, exactly as
with 0.4.0: `key gate-code-page 246810` fails "node cannot receive native focus". The gpui-agent node tree at that
moment (failure-tree.json in macos/03-buwiz-sign-in/):

                    generic 'gate-code-page' enabled=True focused=False
                        generic 'Buwiz Forms' enabled=True focused=False
                          note 'Check your email' enabled=True focused=False
                          generic 'gate-code-sentence' enabled=True focused=False
                            note 'We sent a 6-digit code to' enabled=True focused=False
                            note 'juana@example.com' enabled=True focused=False
                            generic 'gate-code-boxes' enabled=True focused=False
                              textbox 'Sign-in code, 6 digits' enabled=True focused=False
                            generic 'gate-code-slot' enabled=True focused=False
                          button 'Send a new code in 4:59' enabled=False focused=False

The focusable node is the inner textbox ("Sign-in code, 6 digits"), which carries no testID — the testID
gate-code-boxes sits on its non-focusable generic wrapper — so no recipe verb can target it. Nothing else in the
driver's reach changed the outcome (click boxes: not pressable; click by the textbox's name: unsupported target
form; key the page: cannot receive native focus).

known_gaps:
1. The sign-in code cannot be keyed (above; the OTP input's testID is not on the focusable node — a driver or kit
   issue, for the owner to file). tin-chips and tin-queue-sheet are blocked behind it: they need the signed-in
   account. Evidence of everything up to the gap is in macos/ with report.json and report-macos.md.
2. The iPhone simulator proof moves to another machine: the only simulator runtime on the build machine is the
   in-use iOS 27.0 and may not be deleted.
