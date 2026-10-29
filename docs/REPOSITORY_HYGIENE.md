# Repository hygiene review

Before cleanup, the full tracked working tree and all reachable Git refs at
`72b2518` were preserved in a private operator archive. Live attempt directories,
raw recording and accounting/cleanup evidence were separately preserved. No
history was rewritten and no spending reservation was released by this cleanup.

The current tree removes 38 operational JSON receipts: provider runs, billing,
reconciliations, authorization history and image qualification observations.
Ignore rules prevent their ordinary reintroduction. Model artifact contracts,
source pins, build code, test fixtures, licenses and required vendored identity
source remain. Machine-local provenance paths were removed from the vendor note and runbook.
The obsolete operational continuation handoff and historical demo media were
also removed after preservation.
Historical documentation links now identify the archived evidence scope.
The container README's embedded historical billing, resource identifiers,
machine paths and contradictory cutover snapshots were separately preserved
and replaced with a current source/build guide. This changes the current tree
only; the old documentation remains in Git history.

A separate scan inspected 412 blobs reachable from fetched local Git refs. It
checked the explicitly selected Vast key, common GitHub/AWS token patterns and
private-key markers. There were no selected-key or common-token matches. The one
private-key marker was the literal rejection fixture in
`deploy/container/tests/test_release.py`, not private key material. Twenty-eight
historical blobs matched machine-local path patterns. The operator keeps the
per-object results privately; they are not reproduced here.

This is a scoped pattern review, not comprehensive secret clearance. Ignoring or
deleting current files does not remove history. Rewriting history, purging host
caches, or rotating credentials requires a separate concrete decision; none was
performed as part of this PR.
