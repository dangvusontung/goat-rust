# Milestone 6: native capacity measurements

Status: in progress. Native Linux ARM64 access/run pending. No mobile gate passed.

The opt-in `capacity-bench` feature creates 200,000 NPCs across the existing
1,200 clubs, 60 leagues and 20 nations. This measures dense roster pressure,
not future 150-country fixture volume or realistic club economics. Production
world generation and save formats remain unchanged. Diagnostic saves use a
separate GOATCAP1 envelope and checkpoints bind the population/deep profile.

Initial x86_64 sample: seed 42, SIM15, 16 weeks, six permanent deep leagues.
Single warm session RSS 129,232 KiB; save/restore diagnostic peak 374,260 KiB.
Full diagnostic save 63,991,334 bytes, including checkpoint 60,820,559 bytes.
Fresh-process load/query: checkpoint 358,535 us, cold replay 2,159,282 us.
Both paths produced fingerprint 10290559745025018835; in-run checkpoint bytes
also matched exactly. One sample only; not a mobile performance prediction.

Validation: full default workspace quality gate (format, Clippy, tests, career
invariants and 50-seed scanner) passes; three capacity profile tests and
feature-enabled Clippy pass. Annual/long-career measurements and budget comparison
are still pending. The initial progression sample overlapped a short Clippy
run, so it is unsuitable as an isolated CPU timing baseline.

Run on Linux ARM64 with Rust 1.90 and Python 3:

```sh
bash scripts/bench-capacity-native.sh /tmp/goat-capacity-native
```

The script records architecture/toolchain/CPU, builds on the target machine,
and measures 16 weeks plus fresh-process checkpoint/cold loads for deep budgets
1, 3 and 6. Return the environment.txt and measurement .txt files. Diagnostic
.capbench files are not gameplay saves and need not be uploaded.
