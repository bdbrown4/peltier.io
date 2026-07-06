# results/calibration/

Evidence for the bench-runner acceptance criteria (SPEC §3.1):

- **A/A self-tests**: same binary as both sides must yield a null
  verdict; false-positive rate < 5% across sessions.
- **Regression injection**: a synthetic 5% slowdown must be detected
  ≥ 95% of the time.

One JSON file per calibration session, including the full env
fingerprint. These files are the license to trust every number the
system reports afterward; they are referenced from case studies and ROI
reports.
