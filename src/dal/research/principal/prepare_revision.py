"""Make the approved analysis input without changing the source CSV."""
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
SOURCE = ROOT / "src/dal/data/raw/principal/ricepest_survey_combined_66_68.csv"
KNOWN_COORDINATE_ROWS = {102, 522, 553, 884, 1018, 1189, 1442}
UNCERTAIN_LABEL = "ก่อนเก็บเกี่ยวเกี่ยว"
EXPECTED_SOURCE_SHA256 = "8ded458fc671f33660101cf4753b1c5427dde604da391786a4c8212839bdaa9b"


def prepare(output):
    output.mkdir(parents=True, exist_ok=True)
    source_hash = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    if source_hash != EXPECTED_SOURCE_SHA256:
        raise ValueError("Source hash differs. Row-index exclusions must not run on another input.")
    with SOURCE.open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        fields = reader.fieldnames
        rows = list(reader)
    assert len(rows) == 2398 and len(fields) == 42
    detected = set()
    for index, row in enumerate(rows):
        lat, lon = float(row["lat"]), float(row["lon"])
        if not (-90 <= lat <= 90 and 97 <= lon <= 106) or lat == lon:
            detected.add(index)
    assert detected == KNOWN_COORDINATE_ROWS, (detected, KNOWN_COORDINATE_ROWS)
    uncertain = {i for i, r in enumerate(rows) if r["rice_stage"] == UNCERTAIN_LABEL}
    assert len(uncertain) == 21
    kept, excluded = [], []
    for index, row in enumerate(rows):
        reasons = []
        if index in detected:
            reasons.append("known_coordinate_anomaly")
        if index in uncertain:
            reasons.append("uncertain_stage_label")
        if reasons:
            excluded.append({"source_row_id": index, "reason": ";".join(reasons), **row})
        else:
            kept.append({"source_row_id": index, **row})
    assert len(kept) == 2370 and len(excluded) == 28
    for name, values, columns in [
        ("rice_survey_analysis_v2.csv", kept, ["source_row_id", *fields]),
        ("revision_exclusions.csv", excluded, ["source_row_id", "reason", *fields]),
    ]:
        with (output / name).open("w", encoding="utf-8", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=columns)
            writer.writeheader()
            writer.writerows(values)
    summary = {
        "source": str(SOURCE.relative_to(ROOT)),
        "source_sha256": source_hash,
        "source_row_id": "zero-based data-row index; excludes CSV header",
        "source_records": len(rows),
        "coordinate_exclusions": len(detected),
        "uncertain_label_exclusions": len(uncertain),
        "exclusion_overlap": sorted(detected & uncertain),
        "analysis_records": len(kept),
        "analysis_stage_counts": dict(Counter(r["rice_stage"] for r in kept)),
        "analysis_provinces": len({r["province"] for r in kept}),
        "analysis_varieties": len({r["rice_variety"] for r in kept}),
        "analysis_ecosystems": len({r["rice_ecosystem"] for r in kept}),
        "source_date_formats": dict(Counter(
            "excel_serial" if r["survey_date"].isdigit() else "iso" for r in rows
        )),
        "source_omitted_trailing_fields": {
            field: sum(r[field] is None for r in rows)
            for field in fields if any(r[field] is None for r in rows)
        },
        "missing_field_serialization": "An omitted CSV field (DictReader None) is written as an empty field. No observed value is changed.",
        "analysis_ecosystem_counts": dict(Counter(r["rice_ecosystem"] for r in kept)),
        "output_sha256": {
            name: hashlib.sha256((output / name).read_bytes()).hexdigest()
            for name in ["rice_survey_analysis_v2.csv", "revision_exclusions.csv"]
        },
    }
    (output / "revision_data_summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return summary


if __name__ == "__main__":
    print(json.dumps(prepare(ROOT / "src/dal/data/local/reproduction/principal"), ensure_ascii=False, indent=2))
