---
analysisTarget: 'docs/prd.md + docs/architecture.md'
analysisDate: '2026-06-19'
lens: 'over-engineering / scope discipline'
---

# Over-Engineering Analysis — EnvForge v0.9 "Omnipresence"

**Method:** each task scored **Need** (1–5; 5 = essential to the core thesis) and **OE-risk** (1–5; 5 = likely gold-plating / premature / speculative). **Keep score = Need − OE-risk.** Higher = clearer keep; ≤ 0 = trim/defer/cut.

**Core thesis (the yardstick):** *one `envforge fence` covers every AI tool a dev uses, with honest per-tool status — no silent holes.* Anything not serving that is a candidate for cutting.

## Scored tasks

| # | Task | Need | OE | Keep | Verdict |
|---|---|---|---|---|---|
| A | Fence registry refactor (D1/D2, const table + FileKind writers) | 4 | 2 | **+2** | KEEP (with caveat) |
| B1 | Tier-1 tool fence: Copilot, Cursor, Claude, Gemini, Windsurf | 5 | 1 | **+4** | KEEP |
| B2 | OSS-real fence: Cline, Aider | 4 | 1 | **+3** | KEEP |
| B3 | **`AGENTS.md`** cross-tool target | 5 | 1 | **+4** | KEEP (highest leverage) |
| B4 | Long-tail fence: Roo Code, Continue, Augment, Tabnine, Amazon Q | 2 | 4 | **−2** | DEFER → community data |
| C | `fence --status` per-tool + installed-but-unfenced | 5 | 1 | **+4** | KEEP (this IS the thesis) |
| D | `doctor --ai` standalone detection scanner | 3 | 3 | **0** | TRIM → fold into status |
| E1 | MCP server existence (local stdio, read-safe tools) | 3 | 3 | **0** | KEEP but gate hard |
| E2 | **MCP remote: Streamable HTTP + OAuth 2.1 + RFC 8707** | 1 | 5 | **−4** | CUT for v0.9 |
| E3 | MCP tools: `list_keys`, `describe_schema` | 4 | 1 | **+3** | KEEP |
| E4 | MCP tools: `exposure_map`, `canary_scan`, `canary_check` | 2 | 3 | **−1** | TRIM → defer 2 of 3 |
| E5 | NFR-S1 no-secret property/fuzz test | 4 | 2 | **+2** | KEEP (security tool) |
| F | MCP-config LSP lint expansion (additive globs) | 4 | 1 | **+3** | KEEP (cheap) |
| G | Neovim first-party plugin | 3 | 2 | **+1** | KEEP |
| H | Zed = LSP + MCP (no custom UI) | 2 | 2 | **0** | KEEP (already minimal) |
| I | Emacs / Sublime stretch plugins | 1 | 4 | **−3** | CUT from v0.9 |
| J | CI primitives: JSON output + exit codes | 4 | 1 | **+3** | KEEP |
| K | Redaction / audit / round-trip / atomic invariants | 5 | 1 | **+4** | KEEP (non-negotiable) |

## Where the over-engineering is (confident calls)

**1. MCP remote transport + OAuth 2.1 / RFC 8707 (E2) — biggest offender. CUT.**
A local dev secrets tool's MCP server runs over **stdio**. Every shipping precedent the agent will hit (Claude Desktop/Code, Cursor, VS Code) drives MCP servers locally. Building Streamable HTTP + OAuth 2.1 + resource-indicator auth is speculative remote-use scaffolding that (a) adds a large surface, (b) adds a *new network attack surface to a security product*, (c) blocks nothing if omitted. **v0.9 = stdio only.** Revisit remote when a real remote consumer exists. Saves the most effort for zero thesis loss.

**2. Long-tail tool breadth (B4) — chasing declining/tiny adoption. DEFER to data.**
Roo Code, Augment, Tabnine (fading, per research), Continue (acquired by Cursor → uncertain), Amazon Q (opaque, no ignore file anyway). The registry makes these *cheap to add later as community data* — that's the whole point of D1. Shipping them in Band-1 is "over-optimum" optics, not user value. `AGENTS.md` (B3) already covers several of them implicitly. **Ship Tier-1 + OSS + AGENTS.md; long-tail = post-GA data PRs.**

**3. Whole MCP server (E1) is arguably a *separate release* from the fence thesis.**
The fence is the product; the MCP egress is a second, large feature bolted on. Feature-flagging it (`mcp-server`) was the right de-risk — keep that. But be honest in planning: if capacity tightens, the entire MCP server is the clean cut line, and the fence release still delivers the thesis. Don't let MCP gold-plating (E2/E4) delay fence breadth.

**4. `doctor --ai` standalone scanner (D) — partial duplication. TRIM.**
The recovery journey (J2) is served if `fence --status` reports `installed-but-unfenced` using the registry's detection hints. A separate full config-dir-walking `doctor --ai` command is extra surface for marginal gain. **Fold detection into status; skip the standalone command for v0.9.**

**5. MCP bonus tools (E4).** `exposure_map`/`canary_scan`/`canary_check` already exist via CLI/LSP. Exposing all three over MCP day-1 is scope. Ship `list_keys` + `describe_schema` (the actual agent need from Journey 3); defer the other three.

**6. Emacs/Sublime plugins (I).** Correctly Band-2 in the PRD — keep them OUT of v0.9. Flag only because "over-optimum" framing tempts pulling them in.

## Where scope discipline is GOOD (not over-engineered)

- **Registry-as-data (A)** is justified at ~14 targets and is what makes B4 cheap-to-defer. Minor caveat: if it slows the MVP, enum-first then extract — but const-table is low cost, so keep.
- **Dropping Fleet + Zed native UI** — disciplined, evidence-based cuts. Good.
- **Redaction / audit / round-trip / atomic (K)** — mandatory for a security tool, not gold-plating.
- **Feature-flagging MCP** — correct de-risk.
- **NFR-S1 fuzz test (E5)** — proportionate for a secrets product.

## Recommended lean v0.9 (cut/defer list)

**Cut from v0.9:** MCP remote transport + OAuth (E2); Emacs/Sublime plugins (I); MCP `canary_scan`/`canary_check`/`exposure_map` (E4, defer).
**Defer to post-GA community data:** long-tail tools (B4).
**Trim:** `doctor --ai` → fold into `fence --status` (D).
**Net effect:** ships the full thesis (fence-everything + honest status + AGENTS.md + Tier-1/OSS tools + local MCP metadata) with materially less surface, fewer attack surfaces, and the registry still making everything cut here a cheap later add.

## Status: APPLIED (2026-06-19)

All recommendations applied to `docs/prd.md` + `docs/architecture.md`:
- **MCP remote transport + OAuth/RFC 8707 → Vision** (v0.9 = stdio only). FR12, NFR-C4, D6.
- **MCP tools → 2** (`list_keys`, `describe_schema`); `exposure_map`/`canary_scan`/`canary_check` → Vision. FR13/FR14, D5.
- **Long-tail tools (Roo/Continue/Augment/Tabnine/Amazon Q) → Vision/community data.** Band-1 now Tier-1 (Cursor/Copilot/Claude/Windsurf/Gemini) + OSS (Cline/Aider) + `AGENTS.md`. FR2.
- **`doctor --ai` standalone → folded into `fence --status`** (detection via registry hints). FR10, D4.
- **Emacs/Sublime plugins → Vision.** FR23a.
- Measurable Outcomes, scope bands, journeys, NFRs, structure tree all synced.

**Bottom line:** PRD was ~75% lean. The ~25% over-engineering clusters in **MCP remote/auth scaffolding** and **long-tail tool breadth** — both driven by the "over-optimum / release everything" directive. Both are deferrable at near-zero thesis cost because the registry + feature-flag architecture already make them cheap to add later.
