# Architecture Decision Records

An ADR records one architectural decision: the context, the options weighed, what was chosen,
and what that costs. It answers **why**, where a spec or the code answers **what**.

Accepted ADRs are binding constraints, not background reading. If a task requires contradicting
one, stop and say so - propose a superseding ADR instead of quietly working around it.

## Two tiers

This repo's records are the service-local `ADR-NNNN` tier - binding on `game-systems-web` only,
and they die with it. Platform-wide decisions are `PADR-NNNN` in `sweetrpg/platform`'s
`docs/adr/`. Number sequentially within this tier.

## Rules

- **One decision per file. Written once, never rewritten.** When a decision changes, write a new
  ADR and mark the old one `superseded` - do not edit its Context, Options, Decision, or
  Consequences.
- **Immutability applies to accepted and superseded ADRs.** A `proposed` ADR may be revised
  until it flips to `accepted` or `rejected`.
- **Rejected options are mandatory.** Write what was not taken and the concrete reason.
- **Keep the count low.** The trigger is *cost to reverse*, not importance. One-way doors get an
  ADR; anything undoable in an afternoon does not.

## Statuses

`proposed` -> `accepted` | `rejected`; an accepted record can later become `superseded` by a
newer one.

## Frontmatter

```yaml
---
status: accepted
date: 2026-09-07
supersedes: []
superseded-by: []
---
```

## Writing one

Copy `0000-template.md` to `NNNN-short-title.md` and fill every section. `/adr "<title>"`
scaffolds one.

## Index

| ADR | Title | Status |
| --- | --- | --- |
| [0000](0000-template.md) | Template | - |
| [0001](0001-publisher-lookup-via-catalog-api.md) | Publisher lookup via catalog-api | accepted |
