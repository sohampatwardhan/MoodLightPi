# Task 1.1 build/test evidence

Host: node v24.14.0, npm 11.9.0. Resolved: vite 7.3.6, preact 10.29.8,
@preact/preset-vite (see frontend/package-lock.json).

## typecheck — `npm run typecheck` (tsc --noEmit)
Exit 0 (clean).

## build — `npm run build` (vite build)
Exit 0. Emitted:
- web-dist/index.html (0.40 kB / gzip 0.26 kB)
- web-dist/assets/index-<hash>.js (11.57 kB / gzip 4.91 kB)

Bundle JS+CSS gzip well under the 50 kB budget (R15.1); CSS added in task 2.3.

## supply chain — `npm audit`
0 vulnerabilities across 170 resolved packages (2 prod, 169 dev, 53 optional).
Full AuditResult 1.0 evidence: pre-change and post-change reports under
.security/dependency-audit/{pre,post}/. Both exit 0 with 0 findings; gate status
"warnings" reflects only incomplete transitive inventory resolution (npm-list
partial), not a vulnerability — reviewed and accepted.

## unit tests
None in this task; Vitest suite is added in task 3.3.
