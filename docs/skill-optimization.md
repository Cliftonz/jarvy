---
title: "Skill Optimization & Jarvy - Producer/Consumer Model"
description: "How optimizer tools like Microsoft SkillOpt fit with Jarvy's skill distribution: publishers optimize offline, Jarvy verifies and distributes. Jarvy never runs the optimization loop itself."
---

# Skill Optimization (SkillOpt) and Jarvy

Tools like [Microsoft SkillOpt](https://github.com/microsoft/SkillOpt)
treat a skill document (`SKILL.md`) as a trainable parameter: they run an
agent against a benchmark, score the trajectories, generate candidate text
edits with an optimizer model, and accept only edits that strictly improve
validation performance. The output is a compact, evolved `best_skill.md`.

Jarvy sits on the other side of that process. This page defines the
boundary.

## The split: SkillOpt produces, Jarvy distributes

| Concern | Owner |
|---------|-------|
| Evolving skill text (rollout → reflect → update loop) | SkillOpt (publisher-side, offline) |
| Benchmarking / validation gates | SkillOpt (publisher-side) |
| Versioning the winning skill | Publisher's library manifest |
| Integrity (sha pin), trust gates | Jarvy ([library registry](library-registry.md)) |
| Fan-out to all installed agents | Jarvy ([skills](skills.md)) |
| Detecting and applying upgrades consistently | Jarvy (`jarvy skills update`) |

## The upgrade loop

1. **Optimize (publisher, offline).** The skill publisher runs `skillopt`
   (or the nightly `skillopt-sleep` engine) against their benchmark.
   SkillOpt emits an improved `best_skill.md`.
2. **Publish.** The publisher ships that file as the next version of the
   skill in their library manifest: version bump + new sha pin. Nothing
   about the manifest format changes — an optimized skill is just a new
   skill version.
3. **Distribute (consumer).** Users run `jarvy skills update`. The
   `.jarvy-skill.json` sidecar comparison detects the version/sha
   divergence and reinstalls the skill for every configured agent
   (claude-code, cursor, codex, windsurf, cline, continue). Pinned
   versions still refuse mismatches — an optimizer revving a skill never
   overrides an explicit pin.

That third step is what makes upgrades *consistent*: one canonical,
sha-verified source fans out to every agent on every machine, instead of
each agent's skill copy drifting independently.

## Why Jarvy doesn't run the loop itself

Embedding SkillOpt in Jarvy was considered and rejected
(PRD-058 non-goals):

- **Process posture.** The optimization loop is a long-lived training
  process (rollouts, evaluation, an optimizer model). Jarvy is
  deliberately spawn-and-exit — the same reason it has no daemon and the
  freshness advisory's background refresher forks and dies.
- **Trust posture.** Jarvy's skill pipeline is a *verifier and
  distributor*: content is sha-pinned at publish time and verified at
  install time. An in-loop optimizer would make Jarvy a *generator* of
  skill content, destroying the property that what lands on disk is
  exactly what the publisher signed off on.
- **Cost/benefit.** Publishers who want evolved skills can run SkillOpt
  where training belongs — CI, a scheduled job, a workstation — and the
  existing publish path carries the result to consumers with zero new
  Jarvy surface.

## Possible follow-up (advisory provenance)

A future PRD-049 extension may add optional manifest metadata for
optimized skills — e.g. `optimizer = "skillopt"`, an eval score, and the
benchmark name — surfaced by `jarvy skills status`. These fields would be
purely advisory: the sha pin remains the only trust anchor, and Jarvy
would not verify optimization claims.

## One skill body, all agents

SkillOpt's published results indicate optimized skills transfer across
model scales and execution harnesses, so Jarvy keeps its single-body
model: one `SKILL.md` per skill version, fanned to all agents. If
per-agent divergence turns out to matter in practice, a `variants` map
keyed by agent slug in the manifest is the designed escape hatch — not
per-agent optimization runs inside Jarvy.
