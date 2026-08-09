-- Shared SQL conformance corpus for edtf_postgres.
--
-- Run by BOTH paths, so the binary is held to the same standard as the
-- source and the two cannot drift:
--   * the pgrx suite, via Spi::run in src/lib.rs
--   * the prebuilt-tarball smoke test, via psql -f, against a real install
--
-- Deliberately not a second, weaker set of assertions written for the smoke
-- test. "Exercise each function" would duplicate what the pgrx tests already
-- encode and rot within two releases; this is the one source of truth for
-- "does this extension work".
--
-- Every check is a plpgsql ASSERT, so the first failure aborts with the
-- message and a non-zero exit under `psql -v ON_ERROR_STOP=1`.
--
-- NOT in ./sql/ on purpose: pgrx copies ./sql/*.sql into the install tree as
-- extension scripts. This is a test fixture, not part of the extension.

-- Assertions are on by default; a corpus that silently passed because the
-- default had been changed would be worse than no corpus.
SET plpgsql.check_asserts = on;

DO $corpus$
BEGIN
    -- edtf_valid ---------------------------------------------------------
    ASSERT edtf_valid('1985-04-12'), 'level 0 calendar date';
    ASSERT edtf_valid('2004-06~-11'), 'level 2 qualified month';
    ASSERT edtf_valid('{1667,1668,1670..1672}'), 'level 2 set with range';
    ASSERT edtf_valid('1985-24'), 'level 1 season (winter 1985)';
    ASSERT NOT edtf_valid('1985-02-30'), 'no such calendar day';
    ASSERT NOT edtf_valid('19850412'), 'basic format is not EDTF';
    ASSERT NOT edtf_valid('2004/2003'), 'interval must not run backwards';

    -- edtf_level ---------------------------------------------------------
    ASSERT edtf_level('1985-04-12') = 0, 'level 0';
    ASSERT edtf_level('1985~') = 1, 'level 1';
    ASSERT edtf_level('156X-12-25') = 2, 'level 2';
    ASSERT edtf_level('junk') IS NULL, 'level of an invalid expression is NULL';

    -- edtf_canonical -----------------------------------------------------
    ASSERT edtf_canonical('?2004-?06-?11') = '2004-06-11?',
        'per-component qualification collapses to whole-expression form';
    ASSERT edtf_canonical('junk') IS NULL, 'canonical of an invalid expression is NULL';

    -- edtf_min / edtf_max ------------------------------------------------
    ASSERT edtf_min('1985')::text = '1985-01-01', 'year floor';
    ASSERT edtf_max('1985')::text = '1985-12-31', 'year ceiling';
    ASSERT edtf_min('156X')::text = '1560-01-01', 'unspecified-digit decade floor';
    ASSERT edtf_max('156X')::text = '1569-12-31', 'unspecified-digit decade ceiling';
    -- Northern winter wraps into the following (leap) year.
    ASSERT edtf_max('2003-24')::text = '2004-02-29', 'season 24 wraps across the year boundary';
    ASSERT edtf_max('1985-04-12/..')::text = 'infinity', 'open interval end';
    ASSERT edtf_min('../1985')::text = '-infinity', 'open interval start';
    -- Unknown ends and years outside the Postgres date range are NULL,
    -- never a silently clamped date.
    ASSERT edtf_max('1986-04/') IS NULL, 'unknown interval end is NULL, not infinity';
    ASSERT edtf_min('Y17E7') IS NULL, 'year beyond the Postgres date range is NULL';
    ASSERT edtf_min('Y-17E7') IS NULL, 'year before the Postgres date range is NULL';

    -- edtf_relation ------------------------------------------------------
    ASSERT edtf_relation('1985~', '199X') = ARRAY['definitely_before'],
        'approximation never moves bounds, so the relation stays definite';
    ASSERT edtf_relation('198X', '1985') = ARRAY[
        'possibly_before', 'possibly_after', 'possibly_overlaps',
        'possibly_contains', 'possibly_within', 'possibly_equal'
    ], 'overlapping uncertainty yields all six as possible, none definite';
    -- An unknown interval end must never produce a definite verdict.
    ASSERT NOT ('definitely_after' = ANY(edtf_relation('1985/', '../1980'))),
        'unknown ends are never definite';
    -- The documented consistency-rule shape: born must not be after died.
    ASSERT NOT ('definitely_after' = ANY(edtf_relation('1890~', '1976-01-12'))),
        'consistency rule: birth before death';
    ASSERT edtf_relation('junk', '1985') IS NULL, 'relation with an invalid operand is NULL';

    -- Intended usage: index-friendly range overlap ------------------------
    -- Bounds pinned first, deliberately: daterange(NULL, NULL, '[]') is
    -- UNBOUNDED and therefore contains every date, so the containment
    -- assertions below would pass even if both functions returned NULL.
    ASSERT edtf_min('196X')::text = '1960-01-01', 'decade floor';
    ASSERT edtf_max('196X')::text = '1969-12-31', 'decade ceiling';
    ASSERT NOT (daterange(edtf_min('156X'), edtf_max('156X'), '[]') @> DATE '1965-06-15'),
        '1965 is outside the 1560s';
    ASSERT daterange(edtf_min('196X'), edtf_max('196X'), '[]') @> DATE '1965-06-15',
        '1965 is inside the 1960s';
END
$corpus$;
