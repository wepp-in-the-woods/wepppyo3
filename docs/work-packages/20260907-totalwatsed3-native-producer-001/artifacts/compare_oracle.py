"""Strict semantic comparison for totalwatsed3 package evidence."""
import argparse
import json
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq


def compare(oracle_path, actual_path):
    oracle = pq.read_table(oracle_path)
    actual = pq.read_table(actual_path)
    result = {
        "oracle": str(oracle_path),
        "actual": str(actual_path),
        "oracle_rows": oracle.num_rows,
        "actual_rows": actual.num_rows,
        "schema_equal": actual.schema.equals(oracle.schema, check_metadata=True),
        "column_order_equal": actual.column_names == oracle.column_names,
        "missing_from_oracle": [c for c in actual.column_names if c not in oracle.column_names],
        "missing_from_actual": [c for c in oracle.column_names if c not in actual.column_names],
        "differences": [],
    }
    if actual.num_rows != oracle.num_rows:
        result["passed"] = False
        return result
    for name in oracle.column_names:
        if name not in actual.column_names:
            continue
        a, o = actual[name], oracle[name]
        if a.type != o.type:
            result["differences"].append({"column": name, "type_mismatch": True})
            continue
        an, on = a.is_null().to_numpy(), o.is_null().to_numpy()
        if not np.array_equal(an, on):
            result["differences"].append({"column": name, "null_mismatch": True})
        valid = ~(an | on)
        x, y = a.to_numpy()[valid], o.to_numpy()[valid]
        if pa.types.is_floating(a.type):
            equal = np.isclose(x, y, rtol=1e-10, atol=1e-12, equal_nan=True)
        else:
            equal = x == y
        if not equal.all():
            first = int(np.flatnonzero(valid)[np.flatnonzero(~equal)[0]])
            result["differences"].append({
                "column": name,
                "mismatches": int((~equal).sum()),
                "max_abs": float(np.nanmax(np.abs(x - y))),
                "first_row": first,
                "actual": a[first].as_py(),
                "oracle": o[first].as_py(),
            })
    result["passed"] = result["schema_equal"] and not result["differences"]
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle", type=Path)
    parser.add_argument("actual", type=Path)
    args = parser.parse_args()
    result = compare(args.oracle, args.actual)
    print(json.dumps(result, indent=2, allow_nan=False))
    raise SystemExit(0 if result["passed"] else 1)
