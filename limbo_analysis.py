import yaml
import os
import re
import argparse
import sqlite3


def is_not_implemented(result):
    stderr = result.get("stderr", "").lower()
    return (
        "not implemented" in stderr
        or "not supported" in stderr
        or "not yet implemented" in stderr
        or "todo" in stderr
        or "no such module" in stderr
        or "not a valid pragma name" in stderr
        or "no such table: sqlite_stat1" in stderr
        or "only passive mode supported" in stderr
        or "create index is disabled by default" in stderr
        or "not a valid pragma name" in stderr
        or "unknown function" in stderr
        or "enabled only with" in stderr
        or "cannot use expressions in" in stderr
        or "NOT NULL constraint failed" in stderr
    )


def is_update_c0_c0(result):
    checked = result.get("stderr", "")
    pattern = r"column\s+\"c\d+\"\s+specified\s+more\s+than\s+once\s+at"
    return re.search(pattern, checked) is not None


def is_commit_transaction(result):
    checked = result.get("stderr", "")
    return "BEGIN TRANSACTION" not in checked and (
        "COMMIT TRANSACTION" in checked
        or "COMMIT" in checked
        or "END TRANSACTION" in checked
        or "END" in checked
        or "ROLLBACK TRANSACTION" in checked
    )


def is_invalid_step(result):
    stderr = result.get("stderr", "").lower()
    return "step() returned invalid result" in stderr


def is_like_on_nontext(result):
    stderr = result.get("stderr", "").lower()
    return "internal error: entered unreachable code: like on non-text registers" in stderr


def is_invalid_page_type(result):
    stderr = result.get("stderr", "").lower()
    return 'called `result::unwrap()` on an `err` value: corrupt("invalid page type: 1")' in stderr


def is_optimize_no_rewrite(result):
    stderr = result.get("stderr", "").lower()
    return "internal error: entered unreachable code: expression should have been rewritten" in stderr


def is_id_rewrite_as_col(result):
    stderr = result.get("stderr", "").lower()
    return "id should have been rewritten as column" in stderr


def is_header_sz_gt_nr(result):
    stderr = result.get("stderr", "").lower()
    return "assertion failed: (header_size as usize) >= nr" in stderr

def is_invalid_float(result):
    stderr = result.get("stderr", "").lower()
    return "invalid float literal" in stderr

classified_bugs = [
    ("update c0 = c0", is_update_c0_c0),
    ("commit transaction", is_commit_transaction),
    ("invalid step", is_invalid_step),
    ("like on non-text", is_like_on_nontext),
    ("invalid page type", is_invalid_page_type),
    ("optimize no rewrite", is_optimize_no_rewrite),
    ("id rewrite as col", is_id_rewrite_as_col),
    ("header sz gt nr", is_header_sz_gt_nr),
    ("invalid float", is_invalid_float),
]

issues = {
    924: 'called `Result::unwrap()` on an `Err` value: Corrupt("Free block extends beyond page")',
    1040: "error: entered unreachable code: Like on non-text registers",
    1203: "assertion failed: is_empty",
    1629: "INFINITE LOOP",
    1709: "limbo and rusqlite results do not match",
    1731: "Freelist: size is ",
    1734: 'InternalError("row [[',
    1737: 'InternalError("row [[',
    1815: "cell_get: idx out of bounds:",
    1818: "Freelist: size is",
    1975: "expected table or index leaf page",
    1991: "corrupted database, cells were to balanced properly",
    2024: "limbo and rusqlite results do not match",
    2026: "limbo and rusqlite results do not match",
    2047: "overflow cell with divider cell was not found",
    2074: "`Result::unwrap()` on an `Err` value: CacheFull",
    2075: "attempt to shift left with overflow",
    2088: "assertion failed: header_size <= 126",
    2106: "corrupted database, stack is bigger than expected",
    2116: "should return no values for table",
}

def build_log(result) -> str:
    """
    Build a log string from the result dictionary.
    """
    stderr = result.get("stderr", "")
    last_statement_start = stderr.find("java.lang.AssertionError:")
    last_statement_end = stderr.find(";\n", last_statement_start)
    if last_statement_start != -1 and last_statement_end != -1:
        last_statement = stderr[last_statement_start + 26:last_statement_end + 1]
    else:
        raise ValueError("Could not find last statement in stderr")

    first_statement_start = stderr.find("-- Time:")
    if first_statement_start == -1:
        print(stderr)
        raise ValueError("First statement found in stderr, but not expected")

    log = stderr[first_statement_start:] + last_statement + "\n"
    return log

def knowm_bins() -> dict:
    return {
        "update c0 = c0": 0,
        "commit transaction": 0,
        "invalid step": 0,
        "like on non-text": 0,
        "invalid page type": 0,
        "optimize no rewrite": 0,
        "id rewrite as col": 0,
        "header sz gt nr": 0,
        "invalid float": 0,
        "issue-expected": 0,
    }


if __name__ == "__main__":
    p = argparse.ArgumentParser(description="Classify runs per issue into TP_known, TP_unknown, FP, Neg.")
    p.add_argument("--results", required=True, help="Path to root results/ directory")
    args = p.parse_args()

    unknowns = []
    manual_inspections = []
    for run in os.listdir(args.results):
        issue = int(run)
        expected_msg = issues.get(issue, None)
        issue_dir = os.path.join(args.results, run)

        if not os.path.isdir(issue_dir):
            exit(f"Expected {issue_dir} to be a directory, but it is not.")

        bins = {
            "TP_known": [],
            "TP_unknown": [],
            "FP": [],
            "Neg": [],
        }
        known_counts = knowm_bins()

        for i in os.listdir(issue_dir):
            result = {
                "stdout": None,
                "stderr": None,
                "log": None,
            }
            with open(os.path.join(issue_dir, i, "stdout.txt"), "r", encoding="utf-8") as f:
                result["stdout"] = f.read()
            with open(os.path.join(issue_dir, i, "stderr.txt"), "r", encoding="utf-8") as f:
                result["stderr"] = f.read()
            with open(os.path.join(issue_dir, i, "log.txt"), "w+", encoding="utf-8") as f:
                result["log"] = f.read()

            if expected_msg in result["stderr"]:
                bins["TP_known"].append(i)
                known_counts["issue-expected"] += 1
                continue

            if is_not_implemented(result):
                bins["FP"].append(i)
                continue

            is_known = False
            for (name, checker) in classified_bugs:
                if checker(result):
                    if name == "invalid step":
                        # print(f"⚠️  Warning: {name} for issue {issue}, run `./target/debug/tursodb < ../sqlancer/results/limbo/{issue}/{i}/log.txt`")
                        # print(f"stderr: {result['stderr']}")
                        log = build_log(result)
                        try:
                            conn = sqlite3.connect(":memory:")
                            cursor = conn.cursor()
                            cursor.executescript(log)
                            cursor.close()
                            conn.close()
                            manual_inspections.append((issue, i, result, log))
                            bins["TP_unknown"].append(i)
                        except Exception as e:
                            bins["FP"].append(i)
                            # print(f"Error occurred while executing SQL script: {e}")
                    else:
                        bins["TP_known"].append(i)
                        known_counts[name] += 1
                    is_known = True
                    break

            if not is_known:
                unknowns.append((issue, i, result))
                bins["TP_unknown"].append(i)


        print(f"Issue {issue}:")
        print(f"TP_known: {len(bins['TP_known'])}", end=", ")
        print(f"TP_unknown: {len(bins['TP_unknown'])}", end=", ")
        print(f"FP: {len(bins['FP'])}", end=", ")
        print(f"Neg: {len(bins['Neg'])}")
        known_counts = {k: v for k, v in known_counts.items() if v > 0}
        print(f"Known counts: {known_counts}")

    print("\nUnknown issues:")
    for (issue, run, result ) in unknowns:
        print(f"Issue {issue}, run {run}")
        print(f"stdout: {result['stdout']}")
        print(f"stderr: {result['stderr']}")
        print(f"log: {result['log']}")

    print("\nManual inspections needed:")
    for (issue, run, result, log) in manual_inspections:
        print(f"Issue {issue}, run {run}")
        print(f"Log: {log}")
        print(f"stdout: {result['stdout']}")
        print(f"stderr: {result['stderr']}")
        print("Run `./target/debug/tursodb < log.txt` to inspect manually.")

    print(f"\nManual inspections total {len(manual_inspections)}")