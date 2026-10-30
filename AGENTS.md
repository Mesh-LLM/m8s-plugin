# m8s development

## Current standing funding authorization (supersedes earlier session caps)

The user authorizes using the existing funded Vast account credit until it is
exhausted to finish m8s v0 verification and the app demo. The provider's current
available credit is the funding limit; the old $6 session cap and requests for
another $6 are superseded. Do not ask again for per-attempt funding while an
attempt fits the remaining funded credit. No top-ups, new payment methods or
spending beyond that credit are authorized. The selected credential file remains
the workspace `.env` (VAST_API_KEY); never print its value.

Before each paid attempt, observe fresh credit/prices and complete empty instance
and volume inventories. Run at most one rental, bound its full liability within
available funded credit, and arm automatic exact-resource cleanup before CREATE.
Keep historical claims and accounting evidence. Distinguish old conservative
reservations from settled charges already deducted from provider credit: do not
subtract the same charge twice or treat the obsolete session cap as authority.
Reconcile genuine outstanding/unbilled liabilities conservatively. A per-attempt
implementation ceiling is a safety bound, not a new permission requirement.

## Session scope
Use the current user's instructions to determine source, provider and publication
scope. Earlier session restrictions and demo budgets are historical, not standing
authorization or a prohibition on a newly requested live run. A new paid run needs
a current bounded budget and explicitly selected credential file. Never print
credentials or read unrelated credential stores. Tests use deterministic mocks
and synthetic fixtures.

## Historical prototype authorization and continuing safety rules
- User-authorized Vast prototype work may consult public provider documentation and use VAST_API_KEY only from the explicitly selected working-directory .env. Never print the key, put it in argv/logs, copy it into build contexts, or use real credentials in tests.
- The user expanded the cumulative prototype authorization from USD6 to USD16 after personally adding USD10, and authorized continued bounded attempts until proof or funds are exhausted. Keep at most one rented instance at a time; verify prior instance and volume absence before a replacement. Never reset prior spending, consumed claims, or retained reservations. No agent account top-ups or open-ended storage. Establish bounded liability and independently surviving exact-resource destruction before each rental; do not rent while source/image integration is unfinished.
- After prototype proof, the user authorizes integrated source publication to a private joabzzz GitHub repository, then continued ADR and codebase development. Preserve existing commits and work.
- No pushes, publishing, or upstream filing without separate authorization. Tests use deterministic mocks and synthetic fixtures.
- Core has no provider dependencies. Use micro-USD, byte capacities, Mbps and injected seconds.
- Keep secret values out of Debug, Serialize, argv, logs, fixtures and the repository.
- Launch intent precedes effects; uncertainty is not absence; stopped is not destroyed.
- Delete instance rows only after explicit provider-confirmed absence. Scope orphan cleanup by exact owner.
- Certified mode requires certificate verification, served model and owner-routed canary. Explicit token-only v0 may report only token-mode inference verification after its separate gates; never imply owner certification.
- Do not weaken iroh bootstrap/control to plaintext transport. Unimplemented security paths fail closed.
- Run scripts/check (fmt, tests, clippy); max two compiler jobs. Coordinate heavy builds.
- Only integration owner edits root Cargo.toml/Cargo.lock. Workers do not delegate.
- Commit coherent slices; export ignored bundles/m8s-<milestone>.bundle with git bundle create --all.

## Repo-local agent skill
- For explaining, running, or recovering a demo, read `.agents/skills/m8s-demo/SKILL.md`. It links the proven artifacts, fresh-run procedure, and exact cleanup/accounting boundaries.
