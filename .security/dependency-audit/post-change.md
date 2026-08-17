# Dependency Security Audit

**Result:** WARNINGS — review required; this is not a clean audit

## Audit context

| Field | Value |
|---|---|
| Mode | change |
| Completed | 2026\-08\-16T18:39:09\.313934Z |
| Project revision | 34f666ffc8a2294da5aecbefcc1779e6cf29eab5 |
| Inventory fingerprint | 814a1ddf64553b2b21ede85ae3cc2f815928f740080f057c189c62077728726d |
| Inventory completeness | incomplete |
| Stable exit code | 0 |

## Report links

- [Machine-readable JSON](latest.json)

## Source availability

| Source | State | Provenance | Diagnostic |
|---|---|---|---|
| cargo\-audit | not\_applicable | not recorded | ecosystem not present |
| govulncheck | not\_applicable | not recorded | ecosystem not present |
| kev | ok | [source](https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json) | — |
| npm\-audit | ok | not recorded | — |
| osv | ok | [source](https://api.osv.dev/v1) | — |
| pip\-audit | not\_applicable | not recorded | ecosystem not present |

## Inventory

Resolved packages: **119**.

Incomplete evidence:
- npm package &\#x27;@edge\-runtime/vm&\#x27; has no exact version
- npm package &\#x27;@esbuild/aix\-ppc64&\#x27; has no exact version
- npm package &\#x27;@esbuild/android\-arm&\#x27; has no exact version
- npm package &\#x27;@esbuild/android\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/android\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/darwin\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/freebsd\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/freebsd\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-arm&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-ia32&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-loong64&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-mips64el&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-ppc64&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-riscv64&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-s390x&\#x27; has no exact version
- npm package &\#x27;@esbuild/linux\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/netbsd\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/netbsd\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/openbsd\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/openbsd\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/openharmony\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/sunos\-x64&\#x27; has no exact version
- npm package &\#x27;@esbuild/win32\-arm64&\#x27; has no exact version
- npm package &\#x27;@esbuild/win32\-ia32&\#x27; has no exact version
- npm package &\#x27;@esbuild/win32\-x64&\#x27; has no exact version
- npm package &\#x27;@napi\-rs/lzma\-linux\-x64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-android\-arm\-eabi&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-android\-arm64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-darwin\-x64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-freebsd\-arm64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-freebsd\-x64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-arm\-gnueabihf&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-arm\-musleabihf&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-arm64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-arm64\-musl&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-loong64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-loong64\-musl&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-ppc64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-ppc64\-musl&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-riscv64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-riscv64\-musl&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-s390x\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-x64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-linux\-x64\-musl&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-openbsd\-x64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-openharmony\-arm64&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-win32\-arm64\-msvc&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-win32\-ia32\-msvc&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-win32\-x64\-gnu&\#x27; has no exact version
- npm package &\#x27;@rollup/rollup\-win32\-x64\-msvc&\#x27; has no exact version
- npm package &\#x27;@types/debug&\#x27; has no exact version
- npm package &\#x27;@types/node&\#x27; has no exact version
- npm package &\#x27;@vitest/browser&\#x27; has no exact version
- npm package &\#x27;@vitest/ui&\#x27; has no exact version
- npm package &\#x27;fsevents&\#x27; has no exact version
- npm package &\#x27;happy\-dom&\#x27; has no exact version
- npm package &\#x27;jiti&\#x27; has no exact version
- npm package &\#x27;jsdom&\#x27; has no exact version
- npm package &\#x27;less&\#x27; has no exact version
- npm package &\#x27;lightningcss&\#x27; has no exact version
- npm package &\#x27;msw&\#x27; has no exact version
- npm package &\#x27;preact\-render\-to\-string&\#x27; has no exact version
- npm package &\#x27;sass&\#x27; has no exact version
- npm package &\#x27;sass\-embedded&\#x27; has no exact version
- npm package &\#x27;stylus&\#x27; has no exact version
- npm package &\#x27;sugarss&\#x27; has no exact version
- npm package &\#x27;terser&\#x27; has no exact version
- npm package &\#x27;tsx&\#x27; has no exact version
- npm package &\#x27;yaml&\#x27; has no exact version

## Blocking findings (0)

None.

## Warnings (0)

None.

## Excluded findings (0)

None.

## Unclassified findings (0)

None.

## Unmatched decisions (0)

None.

## Remediation and acceptance

No remediation or risk acceptance is recorded because there are no actionable findings.
