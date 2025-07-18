#!/usr/bin/env python3
import os
import csv
import argparse
from collections import defaultdict

def load_bug_maps(csv_path):
    """
    Parse the bug CSV and return two dicts:
      - commit_map: { commit_id: [error_message, ...], ... }
      - issue_map:  { issue:    [error_message, ...], ... }
    """
    commit_map = defaultdict(set)
    issue_map  = defaultdict(set)

    with open(csv_path, newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            issue      = row.get('Issue', '').strip()
            commit_ids = row.get('Commit IDs', '').strip()
            message    = row.get('Error message', '').strip()
            if not message:
                continue

            # Issue → messages
            if issue:
                issue_map[issue].add(message)

            # Commit → messages (handle comma‑separated SHAs)
            for cid in commit_ids.split(','):
                cid = cid.strip()
                if cid:
                    commit_map[cid].add(message)

    return dict(commit_map), dict(issue_map)


def classify_run(stdout_path, stderr_path, exit_code_path, known_messages):
    """
    Return one of: "tp_known", "tp_unknown", "false_positive", "negative"
    """
    # Read files (silently treat missing files as empty)
    try:
        stderr = open(stderr_path, 'r', encoding='utf-8').read()
    except FileNotFoundError:
        stderr = ""
    try:
        stdout = open(stdout_path, 'r', encoding='utf-8').read()
    except FileNotFoundError:
        stdout = ""
    try:
        exit_code = int(open(exit_code_path, 'r', encoding='utf-8').read().strip())
    except Exception:
        exit_code = 0

    slo = stderr.lower()
    slo_out = stdout.lower()

    # 1) False positive?
    if "not yet implemented" in slo:
        return "false_positive"

    # 2) True positive?
    is_bug = (exit_code != 0) or ("panic" in slo or "panic" in slo_out) or ("simulation failed" in slo or "simulation failed" in slo_out)
    if not is_bug:
        return "negative"

    # 3) Among bugs, known vs unknown
    for msg in known_messages:
        mlo = msg.lower()
        if mlo in slo or mlo in slo_out:
            return "tp_known"
    return "tp_unknown"


def main():
    p = argparse.ArgumentParser(description="Classify runs per issue into TP_known, TP_unknown, FP, Neg.")
    p.add_argument('--csv',      required=True, help="Path to bugs CSV")
    p.add_argument('--results',  required=True, help="Path to root results/ directory")
    args = p.parse_args()

    commit_map, issue_map = load_bug_maps(args.csv)
    # Prepare output container
    summary = {}

    for issue in sorted(os.listdir(args.results)):
        issue_dir = os.path.join(args.results, issue)

        if not os.path.isdir(issue_dir):
            continue

        # Read the commit for this issue
        commit_txt = os.path.join(issue_dir, 'commit.txt')
        try:
            commit = open(commit_txt, 'r', encoding='utf-8').read().strip()
            known_msgs = commit_map.get(commit, [])
        except FileNotFoundError:
            print(f"⚠️  Warning: no commit.txt for issue {issue}, skipping")
            known_msgs = issue_map.get(issue, [])


        # initialize counts
        counts = {
            'tp_known':   0,
            'tp_unknown': 0,
            'false_positive': 0,
            'negative':   0,
        }

        # iterate all run-* subdirectories
        for entry in os.listdir(issue_dir):
            run_dir = os.path.join(issue_dir, entry)
            if not os.path.isdir(run_dir) or not entry.startswith('iter-'): # alp change to 'iter-' 
                continue

            stdout_f    = os.path.join(run_dir, 'stdout.txt')
            stderr_f    = os.path.join(run_dir, 'stderr.txt')
            exitcode_f  = os.path.join(run_dir, 'exit_code.txt')
            cls = classify_run(stdout_f, stderr_f, exitcode_f, known_msgs)
            counts[cls] += 1

        # store the 4-tuple
        summary[issue] = (
            counts['tp_known'],
            counts['tp_unknown'],
            counts['false_positive'],
            counts['negative'],
        )

    # print results
    print("\nIssue → (TP_known, TP_unknown, FP, Neg)\n" + "-"*40)
    for issue, tup in sorted(summary.items(), key=lambda x: int(x[0]) if x[0].isdigit() else x[0]):
        print(f"{issue}: {tup}")


if __name__ == '__main__':
    main()
