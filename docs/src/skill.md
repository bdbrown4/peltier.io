# Consume it: the peltier skill

Everything this site describes — equivalence before timing, A/A calibration,
interleaved A/B, the CI-lower-bound rule — is packaged as an
[Agent Skill](https://agentskills.io) you can drop into **any repo** and drive
from **any agent harness that reads the SKILL.md standard**: Claude Code,
OpenAI Codex, GitHub Copilot, VS Code, Cursor, Gemini CLI, opencode, Goose,
Amp, Zed, Factory, ZeroClaw, and others.

The skill does not reimplement anything. It drives the same `bench-runner`
binary the case studies used, from a peltier checkout you point it at. That is
deliberate: a second copy of the statistics would be a second thing that can
lie. The corollary is stated in the skill itself — if it cannot find the trust
layer, or the host cannot run it, **it refuses to produce a number** rather
than fall back to a hand-rolled timing loop. A refusal ends the claim, not the
work.

## Install

```sh
# from your repo, with a peltier checkout at /path/to/peltier.io
sh /path/to/peltier.io/scripts/install-skill.sh .
export PELTIER_HOME=/path/to/peltier.io
```

One command stamps both discovery paths, because harnesses look in different
places:

| Path installed | Read natively by |
|---|---|
| `.claude/skills/peltier/` | Claude Code, GitHub Copilot, VS Code, opencode, Amp (and Cursor in compat mode) |
| `.agents/skills/peltier/` | Codex, Zed, Cursor, Gemini CLI, Goose, Copilot, opencode, Amp |

Two copies is a drift risk, so this repo treats it the way it treats every
other claim: **CI enforces the copies stay byte-identical** (`diff -r` in the
`skill-preflight` job), and the same job checks both against the Agent Skills
spec rules (name/directory match, description and body limits) on every pull
request.

## ZeroClaw: the script-blocking case

ZeroClaw's skill security policy blocks script files inside skills by default
(`skills.allow_scripts` is off) — its auditor rejects the full skill on sight.
The installer emits a variant for it:

```sh
sh /path/to/peltier.io/scripts/install-skill.sh --zeroclaw-variant /tmp/peltier-skill
zeroclaw skills install /tmp/peltier-skill
```

The variant simply omits `scripts/`. Nothing is lost and nothing is forked:
`preflight.sh` still exists exactly once — in the peltier checkout the skill
already requires — and the skill's instructions run it from there. Transcribing
or re-implementing the preflight logic inline is explicitly named a refusal
condition in the skill, so a harness's restrictions cannot pressure an agent
into forking the trust layer.

## What the skill enforces

Two modes. `verify` works anywhere, on any two shell commands, in any
language; `attempt` drives the full in-repo loop this site's case studies
document. In `verify` mode, the gate order is the content:

1. **Preflight** — find the trust layer, build the real `bench-runner`, refuse
   on unsupported hosts. `STATUS=ok` means the harness can *run*, not that the
   host can *measure* — only calibration decides that.
2. **Equivalence before timing** — a faster program that computes something
   different is a bug with good benchmarks, not an optimization. Comparing two
   empty outputs is a failed gate, not a passed one.
3. **A/A calibration** — the same command on both sides must yield a null
   verdict. A harness that "finds" a speedup between a binary and itself is
   measuring noise, and every number it produces afterwards is fiction.
4. **Interleaved A/B, bootstrap CI** — and the claim is the **CI lower bound**,
   never the median. A median of 1.088 with 95% CI [1.074, 1.100] (the shipped
   cJSON batch win, on its 5.6 MB / 20k-record parse+serialize workload)
   supports "≥7.4% faster" — not "8.8% faster".
5. **Risk routing** — concurrency, `unsafe`, and floating-point-reordering
   changes are `needs-human-review` regardless of the numbers. In `verify`
   mode the *agent* is instructed to be the classifier, because the standalone
   harness only ever emits `accepted`/`rejected-bench`.

## The platform truth

Measurement requires a Linux/POSIX host — the same requirement as the rest of
the pipeline. On Windows (or with no checkout reachable) the skill refuses and
says why. That is asserted, not assumed: on every pull request, CI runs the
preflight's refusal path (skill copied outside any checkout) and its resolve
paths (in-checkout and via `PELTIER_HOME` from a foreign repo), alongside a
live plumbing run of the harness it drives.

The skill's own files are the reference:
[`SKILL.md`](https://github.com/bdbrown4/peltier.io/blob/main/.claude/skills/peltier/SKILL.md),
[`references/verify.md`](https://github.com/bdbrown4/peltier.io/blob/main/.claude/skills/peltier/references/verify.md),
[`references/attempt.md`](https://github.com/bdbrown4/peltier.io/blob/main/.claude/skills/peltier/references/attempt.md).
