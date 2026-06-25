# `ERR_IO` + `ERR_TIMEOUT` enum constants collide ecosystem-wide — namespace `YuktiErrorKind` as `YUKTI_ERR_*`

**Filed:** 2026-06-23 (by a hoosh consumer — hoosh 2.4.7 toolchain bump to cyrius 6.2.37)
**Severity:** Medium — `last-definition-wins` build warning today; latent
value-dependent-logic hazard when yukti is compiled alongside another lib that
also defines bare `ERR_IO` / `ERR_TIMEOUT`.
**Component:** `src/error.cyr` (`enum YuktiErrorKind`): `ERR_TIMEOUT = 9`
(`:20`), `ERR_IO = 14` (`:25`) → `dist/yukti.cyr:72,77`.
**yukti's role: FIX OWNER for its own error enum.** Part of a coordinated
ecosystem-wide error-enum namespacing effort (see Cross-references).
**Repos:** yukti `2.2.6` (mirrors filed in sigil, bote, sakshi, ai-hwaccel).

## Summary

Cyrius enum members are **global constants** — `YuktiErrorKind` does *not*
namespace them. yukti contributes **two** of the colliding bare names:

| Symbol | yukti | other definers (different values) |
|---|---|---|
| `ERR_IO` | **14** (`src/error.cyr:25`) | sigil 6, bote 11 |
| `ERR_TIMEOUT` | **9** (`src/error.cyr:20`) | sakshi 5, ai-hwaccel 3 (and `SANDHI_ERR_TIMEOUT` already prefixed) |

(`ERR_PARSE = 15` also collides with bote's `ERR_PARSE = 4` — another reason to
prefix the whole enum rather than the two named members.)

Cyrius include semantics are textual paste + **last-definition-wins (with a
warning)**: a consumer including yukti next to sigil/bote/sakshi/ai-hwaccel gets
ONE global `ERR_IO`/`ERR_TIMEOUT` — whichever bundle is included last.

## Why this is more than a warning

After last-wins there is a single value per name in the binary, so intra-module
comparisons stay self-consistent. The latent hazard is **value-dependent logic**:
serializing the numeric code, indexing a table by it, or mapping it across a
module boundary silently uses another lib's integer (e.g. yukti's `ERR_TIMEOUT`
documented as `9` resolving to sakshi's `5`).

## The precedent already exists in-tree

The stdlib already namespaces exactly these — `TLS_ERR_IO`, `PATRA_ERR_IO`,
`SANDHI_ERR_TIMEOUT`. yukti should follow the same convention.

## Recommended fix

Prefix the **entire `YuktiErrorKind` enum** `ERR_* → YUKTI_ERR_*` and update all
internal references (`yukti_err_new` and every `ERR_*` use under `src/`).
Regenerate `dist/yukti.cyr`. Breaking change to the exported error surface →
suggest **yukti 2.3.0**, optionally keeping bare aliases for one minor.

## Interim (consumer-side)

Consumers tolerate the warning today (last-wins benign for reachable paths). The
upstream rename retires the warning + latent hazard for all multi-lib consumers.

## Cross-references

- sigil `…2026-06-23-err-io-enum-collision-namespace.md`.
- bote `…2026-06-23-err-io-enum-collision-namespace.md`.
- sakshi / ai-hwaccel `…2026-06-23-err-timeout-enum-collision-namespace.md`.
- Precedent: bote × ai-hwaccel `registry_new` collision (`2026-06-11-registry-new-collision.md`).
