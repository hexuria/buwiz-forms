# nsh-run-agent Windows timing evidence

Before CSVs are the loops from the first timing investigation. justfile, scripts/tests/test_nsh_run_agent_windows.py, and .github/workflows/ci.yml are byte-identical between that checkout (0265dc6103482b201d017ea512f3700918fb927e) and this branch base (97a96fb70c78a9681318d52396c0c7b938c75b6d). After CSVs are this branch. Loaded rows are four concurrent fresh worker loops, 30 full-class runs each.

| evidence | class runs | test invocations | failed tests | runs with failures | slowest test (s) |
|---|---:|---:|---:|---:|---:|
| after-baseline-class-30.csv | 30 | 300 | 0 | 0 | 7.956 |
| after-loaded-class-1-30.csv | 30 | 300 | 0 | 0 | 8.151 |
| after-loaded-class-2-30.csv | 30 | 300 | 0 | 0 | 8.082 |
| after-loaded-class-3-30.csv | 30 | 300 | 0 | 0 | 8.073 |
| after-loaded-class-4-30.csv | 30 | 300 | 0 | 0 | 8.166 |
| before-baseline-class-30.csv | 30 | 300 | 3 | 3 | 11.120 |
| before-loaded-class-1-30.csv | 30 | 300 | 2 | 2 | 14.609 |
| before-loaded-class-2-30.csv | 30 | 300 | 2 | 2 | 14.673 |
| before-loaded-class-3-30.csv | 30 | 300 | 3 | 3 | 14.584 |
| before-loaded-class-4-30.csv | 30 | 300 | 2 | 2 | 14.090 |

The before single-test loops are also included: `test_android_sdk_root_is_used_when_android_home_is_unset` failed once in 100 runs and `test_a_cancelled_launch_stops_the_token_writer` passed 100 of 100. `before-failure-details.txt` shows the Stop-Job/Receive-Job errors. `normal-timeline-after.txt` is a normal traced run; `forced-timeout-example-after.txt` is the new diagnostic failure output.
