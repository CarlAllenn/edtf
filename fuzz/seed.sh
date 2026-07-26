#!/usr/bin/env bash
# Populate the fuzz seed corpora from the checked-in fixtures: every EDTF
# string in the legacy conformance corpus (built from the Annex A examples),
# the golden snapshot, and the LoC interop corpus. Both targets share the
# same seeds.
set -euo pipefail
cd "$(dirname "$0")"

python3 - << 'EOF'
import hashlib
import json
import pathlib

root = pathlib.Path("..") / "tests" / "fixtures" / "legacy"
seeds = set()

corpus = json.loads((root / "edtf-conformance-corpus.json").read_text())
for section in corpus.values():
    if not isinstance(section, list):
        continue  # metadata keys
    for case in section:
        if isinstance(case, dict) and "edtf" in case:
            seeds.add(case["edtf"])

golden = json.loads((root / "edtf-golden.json").read_text())
seeds.update(golden.keys())

loc = json.loads(
    (root.parent / "loc" / "loc-edtf-examples.json").read_text()
)
for case in loc["examples"] + loc["invalid"]:
    seeds.add(case["edtf"])

for target in ("parse", "roundtrip"):
    out = pathlib.Path("corpus") / target
    out.mkdir(parents=True, exist_ok=True)
    for seed in seeds:
        data = seed.encode()
        (out / hashlib.sha1(data).hexdigest()).write_bytes(data)

print(f"wrote {len(seeds)} seeds per target")
EOF
