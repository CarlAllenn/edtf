# Coverage

Coverage is a **ratchet**, owned by the org toolbelt rather than by this
repository. `.coverage-floor` is derived state: the release machinery
re-derives it at every release as `floor = measured - band`, and the floor
only ever rises.

```bash
mise run coverage:check   # the gate: refuses a measurement below the floor
mise run coverage:report  # writes lcov.info for the Codecov badge feed
```

Do not hand-edit `.coverage-floor`. `coverage:check` refuses a floor that
disagrees with the record inside the file, and reports it as drift rather
than repairing it. To adopt a new measurement deliberately, run
`mise run coverage:adopt`; to step the floor DOWN, replace the derivation
lines with `reset: <reason>`, which a reviewer then sees.

## What is measured, and what is not

`edtf-postgres` is excluded, declared as `COVERAGE_EXCLUDE` in `mise.toml`.
The reason is the same one that keeps it out of `test` and `lint:rust`: its
`pg14`–`pg18` features are mutually exclusive and it needs an initialised
`$PGRX_HOME`, so `cargo llvm-cov` over the workspace has no data for it and
would report a false gap.

That is not a hole in the evidence, it is a different instrument. The
extension is built and tested for real at RELEASE time, by the canon's
pgrx-extension class: every declared Postgres major, in its own container,
plus the `ALTER EXTENSION UPDATE` reachability proof that
`lint:pg-upgrade-path` also runs in the gate.

## The number

The floor is a line coverage percentage measured by `cargo llvm-cov
--workspace --locked --exclude edtf-postgres` — the stable toolchain, no
`--branch`. A previous incarnation of this repository asserted a stricter
invariant (zero uncovered lines and zero untaken branch sides, measured
under a pinned nightly). That measurement is not what gates now; if it is
wanted again it belongs in a repo task beside the belt's, not instead of it.
