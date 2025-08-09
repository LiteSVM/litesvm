import json
import argparse
from tabulate import tabulate
from pathlib import Path

def fmt(num):
    return f"{num:,}"

def fmt_diff(num):
    sign = "+" if num > 0 else ""
    return f"{sign}{num:,}"

def main():
    parser = argparse.ArgumentParser(description="Display CU logs in table format.")
    parser.add_argument(
        "json_path",
        type=Path,
        help="Path to cu_logs.json file"
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=3,
        help="Number of recent runs to display (default: 3)"
    )
    args = parser.parse_args()

    try:
        with args.json_path.open() as f:
            data = json.load(f)
    except FileNotFoundError:
        print(f"File not found: {args.json_path}")
        return
    except json.JSONDecodeError as e:
        print(f"Invalid JSON: {e}")
        return

    data = data[:args.limit]

    for d in data:
        entries = d["entries"]

        rows = [
            (
                k,
                fmt(v["value"]),
                fmt_diff(v["diff"]),
            )
            for k, v in sorted(entries.items())
        ]

        print()
        print("## Run at", d["timestamp"])
        print()
        print(tabulate(rows, headers=["Instruction", "Compute Units", "Diff"], tablefmt="github"))
        print()

if __name__ == "__main__":
    main()
