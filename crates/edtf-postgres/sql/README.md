# Extension upgrade scripts

Hand-written `ALTER EXTENSION edtf_postgres UPDATE` paths. pgrx copies
`*.sql` from this directory into the install tree (and only these files); it
has no ability to generate them.

## Naming

```text
edtf_postgres--<from>--<to>.sql
```

Postgres builds a graph from the available scripts and walks the shortest
path from the installed version to `default_version`, so one script per
release — joining the previous version to the new one — keeps every earlier
version reachable. Verify with:

```sql
SELECT * FROM pg_extension_update_paths('edtf_postgres');
```

A `NULL` path means those two versions are not connected.

## When a script may be empty

`default_version` is `@CARGO_VERSION@`, so every release mints a new
extension version whether or not the SQL surface moved. Because
`module_pathname` is set in the control file, pgrx's versioned-`.so` mode is
off: the library is replaced in place and existing function definitions keep
resolving through `MODULE_PATHNAME`. A release that did not change the SQL
surface therefore needs only an **empty** script — the file's existence is
what creates the path.

`task pg:schema-snapshot` is what tells you whether the surface actually
moved; a clean diff means an empty script is correct, and a non-empty diff
is the list of what the script has to do.

## Why this is not optional

The README recommends these functions inside `CHECK` constraints and
expression indexes. Without an upgrade path a user's only route to a new
version is `DROP EXTENSION`, which either errors on those dependencies or,
with `CASCADE`, silently drops their constraints and indexes.

`assert-upgrade-path.sh` enforces that every version is a target, from the
first script onwards.
