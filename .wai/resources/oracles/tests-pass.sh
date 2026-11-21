#!/usr/bin/env bash
# tests-pass — gate oracle for tdd-ro5: run the DDL standard test runner
# (DDL-85p): `just test` = testaruda select/exec loop over the working diff.
# Tolerance contract: exit 0/10/20 = green (10/20 are testaruda's
# no-tests-selected / vacuous-pass codes, calibrated by testaruda itself);
# anything else blocks. Stderr carries the failure reason.
set -uo pipefail

# no args used: the artifact path is ignored — this oracle validates the
# repo's test state, not the artifact content
just test
code=$?
case $code in
0 | 10 | 20) exit 0 ;;
*)
	echo "tests-pass oracle: just test failed (exit $code)" >&2
	exit "$code"
	;;
esac
