# Six-stage research revision: reproducibility and evaluation

The revised analysis retains 2,370 public rice-survey records. It excludes seven coordinate anomalies and 21 uncertain stage labels.
The original data, deployed model, and historical hardware report remain unchanged.

## Run the study

Use the existing research environment and the approved retained CSV.
The output directory must remain separate from the deployed model directory.

```bash
.venv/bin/python -m src.domain.services.revision_study \
  --survey /path/to/rice_survey_analysis_v2.csv \
  --out src/dal/data/processed/revision_v2 --phase all

TCC_REVISION_FIXTURE="$PWD/src/dal/data/processed/revision_v2/feature_equivalence_fixture.json" \
  cargo test --release --test revision_feature_equivalence_test -- --nocapture
```

Individual phases are `prepare`, `fixture`, `evaluate`, `scouting`, and `export`.
The preparation phase reads the archived NASA POWER cache. It makes no network requests.
Every requested window must be complete. Missing weather stops preparation.

## Measured accuracy

Each pooled score uses saved out-of-fold predictions for all 2,370 records.
The five-fold stratified split uses seed 42. Province grouping evaluates a different transfer target.
Training folds determine weights and category maps. Fixed iteration counts replace outer-fold early stopping.

| Evaluation | Pooled Macro-F1 |
|---|---:|
| CatBoost, six stages | 0.592520 |
| XGBoost, six stages | 0.618860 |
| Random Forest, six stages | 0.587663 |
| CatBoost, three phases | 0.760904 |
| CatBoost, held-out provinces | 0.317710 |
| CatBoost with scouting fields, six stages | 0.663384 |
| Numerical CatBoost export configuration, six stages | 0.603768 |

The original threshold remains 0.75. The three-phase score exceeds it; the six-stage scores do not.
The scouting comparison adds 35 same-survey fields to the primary 44 inputs and reuses the same folds.
Its Macro-F1 difference is 0.070864. This association does not establish causation or climate-only deployment accuracy.
Fifteen nonnumeric values in numeric scouting columns become missing values. Their original text and row IDs are recorded separately.

## Conversion and feature checks

The final numerical configuration uses 350 trees and has its own held-out evaluation.
The export check covers 30 observed rows and one unknown-category case.
All 31 Python/ONNX labels agree. Maximum probability difference is 6.787e-7, below the 1e-5 tolerance.

The focused Rust test compares 56 cases across all 44 inputs and checks future-weather exclusion.
Maximum absolute difference is 0.0100586. Rounded summaries and their empirical-index derivatives allow 0.0101; other inputs allow 0.0001.

## Hardware evidence boundary

No new Raspberry Pi benchmark was run.
The historical report `04_rust_edge_inference_benchmark.md` describes the original model, SHA-256
`bedc319209cc5817e31b355654ff54e4581b64e972d2cfa191bf852e2b27cf53`.
Its measured mean was 56.79 microseconds; maximum round p99 was 80.04 microseconds.
Those measurements must not be transferred to the new six-stage artifact.
The new model's Pi-specific latency remains unmeasured. Server accuracy and conversion checks do not establish it.

## Saved evidence

The output directory contains feature/cache manifests, fold maps, row-level predictions/probabilities,
per-fold parameters and class supports, pooled metrics, scouting-quality logs, SHAP summaries,
the isolated ONNX candidate, encoding maps, conversion checks, and cross-language fixtures.
The manifest records versions and source hashes. The baseline source snapshot remains part of the local evidence archive.
Scores were independently recalculated from all seven prediction files.
