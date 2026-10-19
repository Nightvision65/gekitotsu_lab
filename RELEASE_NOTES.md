# gekitotsu_lab 0.6.6

Lab version: 0.6.6. Embedded GekitotsuKit version: 1.9.1.

- Fix gravity auditing with static-support-gravity-v2: permit normal support pushes up to 35 px, intentional target-ball (ID42) falls and movement within tolerance. Convergence remains required when publishing an applied stable code.
- Align bundled JNI and reference Kit HP with catalog-hp-v1: blue accelerator 100, elevator wall 75. Use a distinct native engine identity to separate legacy caches.
- Enforce strict 200-match JNI/plain/basic/detailed result comparisons and ship reusable Java/JNI HP checks.
- Standardize reported win rates: draws and frame-limit unresolved outcomes count as half a win, with a fixed denominator; errors remain failures.

Source and Windows x64 portable packages contain public code, documentation and fixed test inputs. Portable GUI inputs start empty. Private research, formations, results, caches, local configuration and maintenance materials are excluded.

Missing upstream telemetry/frames goldens remain explicitly skipped; simulator checks do not establish original Flash equivalence.
