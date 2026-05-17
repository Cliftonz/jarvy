# Gitpod (now Ona) — Competitor Profile

**URL**: https://gitpod.io (redirects to https://ona.com)
**Generated**: 2026-05-17
**Depth**: quick scan
**Relationship to Jarvy**: Adjacent. Gitpod was a CDE; as of September 2025 it has re-founded itself as **Ona**, an AI software-engineering agent platform. Different layer than Jarvy, but a meaningful market signal — the CDE category is being absorbed by the "background agent infrastructure" category.

---

## At a Glance

| Metric | Value |
|--------|-------|
| Current name | **Ona** (rebranded from Gitpod, Sept 2025) |
| Tagline | "Run a team of AI software engineers in the cloud. Orchestrated, governed, secured at the kernel." |
| Category | AI software-engineering agent platform + cloud dev environments |
| Legal entity | Gitpod GmbH, Kiel, Germany (HRB 22228) |
| Managing Directors | Moritz Eysholdt, Kai Klasen |
| CEO / public face | Johannes Landgraf |
| Founded | ~2020 (as Gitpod) |
| Scale claim | 2M developers used Gitpod; 440K newsletter subscribers |
| Enterprise customers | BNY Mellon, GSR, Vanta, Pearson, EquipmentShare, Hargreaves Lansdown, Kingland (named on site); also referenced talks from Stripe, Cloudflare, Uber, Harvey |
| Pricing | Free + Core from $20/mo + Enterprise custom; priced in **Ona Compute Units (OCUs)**, pooled per org |
| Open source | Yes — `gitpod-io/gitpod` historically; new Ona platform's OSS status less clear post-rebrand |
| Compliance | GDPR, SOC 2, "Fortune 500" referenced |
| Big architectural move | Left Kubernetes (after 6 years) for custom orchestration |
| Lifecycle event | **Gitpod Classic sunset Oct 15, 2025** |
| Domain rank / traffic | Not pulled — DataForSEO MCP not available |

---

## Positioning & Messaging

**Primary value proposition (current)**:
> "Mission control for your personal team of software engineering agents." Delegate well-scoped tasks → Ona writes code, runs tests, opens a PR. Each agent runs in its own sandboxed cloud environment with kernel-level guardrails.

**Old positioning (Gitpod Classic, sunset)**: One-click cloud dev environments / Codespaces alternative.

**Target audience**: Engineering leaders + platform teams at large enterprises adopting AI coding agents. Strong tilt toward regulated industries (banks, pharma, sovereign wealth, defense-adjacent).

**Positioning angle**: "Self-hosted but vendor-managed" — they explicitly frame this as the wedge against Coder (whom they call "self-hosted and self-managed"). The pitch is: keep your data in your VPC, but let us run the platform so your platform team doesn't carry Day-2 ops.

**Key messaging themes**:
- Background agents need a *real computer*, not a sandbox API call ("The last year of localhost")
- Pattern 1 (agent gets full VM + dev container) vs Pattern 2 (agent calls remote sandbox via API) — they champion Pattern 1
- Kernel-level / OS-level isolation, deterministic guardrails
- 4x developer productivity (customer-reported)
- 60% of their own PRs co-authored by Ona Agents

---

## Product & Features

### Three-pillar architecture
- **Ona Environments** — ephemeral, API-first cloud workspaces. Declarative via `devcontainer.json` + `automations.yml`. Pre-configured with tools, repos, secrets, DB connections. Spin up <60s.
- **Ona Agents** — background SWE agents running inside Ona Environments. Private LLM access + MCP. Slash commands for codified workflows. Conversational interface, can also hand off to VS Code (web or desktop).
- **Ona Guardrails** (Enterprise) — kernel-level command deny lists, egress control, scoped creds, RBAC, SSO/OIDC, full audit trail of every human and AI action. Models via AWS Bedrock, Google Vertex, or private APIs.

### Notable differentiators
- **Left Kubernetes** — built custom orchestration layer for dev-env workloads after 6 yrs of running on K8s; they market this as the reason their installs are "minutes not days"
- **Kernel-level enforcement** of agent guardrails (deterministic, not prompt-based)
- **OCU pricing model** unifies agent token cost + environment compute cost in one credit
- **API + SDK access** at Enterprise tier — programmatic agent orchestration
- Mobile support — start on phone, finish on laptop
- Native devcontainer + automations support
- LLM-discoverable: ships `llms.txt`, `llms-full.txt`

### IDEs / integrations
- VS Code Web (in-browser) + desktop VS Code, JetBrains, Vim, Cursor, Claude Code, Windsurf, Copilot
- GitHub, GitLab, Bitbucket
- AWS, GCP, Amazon Bedrock, Google Vertex
- MCP server support across all tiers
- MongoDB, Redis, generic DB connections inside environments

### Product direction signals (last 12 months)
- **Sept 2025**: Rebrand Gitpod → Ona; pivot to AI agent platform
- **Oct 15, 2025**: Gitpod Classic (the original CDE) **sunset**
- **March 2026**: Launched "Veto" — agent security product
- **Feb 2026**: "Ona for Open Source" program
- Heavy investment in background-agent narrative ("Software Factory" livestream, Background Agents Summit)

---

## Pricing

| Tier | Price | Compute | Seats | Notable |
|------|-------|---------|-------|---------|
| Free | $10 free credit (40 OCUs one-time) | 4 cores / 16GB / 80GB; 3 parallel envs | 1 | Hosted on Ona Cloud; auto-delete after 3 days idle |
| Core | from $20/mo (80–2,200 OCUs/mo) | up to 32 cores / 128GB / 200GB; GPU support | up to 100 (pooled) | Add-on OCUs from $10 per 40 OCUs; prebuilds; RBAC; 7-day auto-delete |
| Enterprise | Custom | Custom (VPC-deployed, AWS or GCP) | Custom | Self-hosted in customer VPC, Ona-managed; warm pools, audit, SSO, SDK, SLA, dedicated forward-deployed engineer |

**Billing model**: OCU-based (unified token + compute credit, pooled across org).
**OCU examples**: 1 OCU = 1hr of 4vCPU/16GB VM, or 1 explain-small-codebase call. 5–8 OCUs = bug fix / feature add on a medium-to-large codebase. GPU VM = 7 OCUs/hr.
**Free trial**: Yes, $100 credits on signup (referenced on homepage).
**Notable**: Per-seat is *not* charged on Core — only OCU consumption. Up to 100 team members included.

---

## AI Agents Posture

**Verdict: agents are the product.** Ona is no longer a CDE that supports agents — it is an AI software-engineering agent platform that happens to ship a CDE underneath. The rebrand explicitly re-founded the company around this thesis.

**What's shipped**:
- **Ona Agents** — flagship product line. Background SWE agents that scope issues, write code, run tests, open PRs. Conversational interface + handoff to VS Code or desktop IDE.
- **Ona Environments** — every agent runs in its own ephemeral, sandboxed cloud env (Pattern 1: full VM + dev container, *not* Pattern 2: sandbox-as-tool API)
- **Ona Guardrails** (Enterprise) — kernel-level, deterministic denial-based guardrails enforced at the environment layer; not prompt-based, cannot be overridden by the agent
- **Veto** — separate agent-security product launched March 2026
- **MCP server support** across all tiers (Free → Enterprise)
- **Slash commands** — codified workflows shareable across team (e.g., `/review-like-mads`)
- **Templates** library at `ona.com/templates` — dependency updates, test coverage, code review, etc.

**Models supported**: AWS Bedrock, Google Vertex, private APIs. References to Claude Code, Cursor, Windsurf, Copilot as compatible.

**Internal proof of progress (their claim)**: Ona Agents co-authored 60% of merged PRs and 72% of merged LOC in their own product engineering org. Customer-reported 4x throughput.

**Pricing implication**: OCUs unify agent token cost + environment compute. A single feature-add task on a medium codebase = 8 OCUs. Budgeting is consumption-based, not seat-based.

**Architectural thesis** (their words): "Background agents cannot run on a laptop. The development environment is the bottleneck." See blog post "The last year of localhost."

**Implication for Jarvy**:
- Ona's thesis is *the* direct contradiction of Jarvy's. If they're right, Jarvy's market shrinks. If they're wrong (and most local agent tooling adoption suggests laptops are still where dev work happens), Ona has built a heavy platform for a niche.
- Jarvy's agent story should *not* try to compete here. Trying to be a remote agent platform is suicidal. Instead: install local agent CLIs cleanly, manage their auth, and let the developer choose where the agent runs.
- A useful framing: Ona = office building for agents. Jarvy = office setup for the human developer (who may or may not also use cloud agents).

---

## Customers & Social Proof

**Named enterprise customers (pricing page)**: BNY Mellon, GSR, Vanta, Pearson, EquipmentShare, Hargreaves Lansdown
**Named case study**: Kingland (enterprise data management; CMMI 5)
**Talks referenced**: Stripe, Cloudflare, Uber, Harvey
**Sectors**: banking, financial services, pharma, sovereign wealth (per rebrand announcement)
**Quote**: Kingland Enterprise Architect: "Our developers used to lose thousands of hours a year fixing broken environments. With Ona, that number is zero."
**Internal proof**: Ona Agents co-authored 60% of merged PRs and 72% of merged LOC inside their own engineering org.
**Compliance**: GDPR, SOC 2.
**Review ratings**: Not pulled in this run.

---

## SEO & Content Strategy

**Not pulled** — DataForSEO MCP not available.

Observed from site structure:
- Domain migration in flight: `gitpod.io` URLs 301 to `ona.com` paths — significant SEO authority transition risk
- They publish `llms.txt` / `llms-full.txt` for AI-search optimization
- Content hubs: blog/stories, events, screencasts (Stripe/Cloudflare/Uber/Harvey talks), templates, customer stories, newsletter
- Active comparison pages: vs Codespaces, vs Coder
- Heavy thought-leadership content: "We're leaving Kubernetes", "The last year of localhost", "Don't build your own sandbox", champion-building playbooks

---

## Strengths & Weaknesses

### Strengths
- **First-mover on the "managed CDE for AI agents" category** with strong narrative discipline
- **Operational simplicity wedge** — vendor-managed VPC eliminates the Day-2 ops burden of Coder/self-hosted
- **Enterprise traction** in regulated industries (BNY, Vanta, Hargreaves Lansdown, GSR, Kingland)
- **OCU pricing** is novel and de-risks per-seat math for orgs that don't want to license inactive devs
- **Strong content engine** — newsletter at 440K subs, conferences, summits
- **Real product progress on agents** — internal usage metrics (60% PR co-authorship) are credible signal, not just marketing
- **Kernel-level guardrails** is a credible technical differentiator for security-sensitive buyers

### Weaknesses
- **Sunset of Gitpod Classic** broke trust with existing CDE users — many migrated to Coder or Codespaces
- **Brand reset risk** — abandoning "Gitpod" loses years of SEO + word-of-mouth equity; ona.com is a new domain
- **Heavy bet on a single thesis** — "agents need full VMs, not sandboxes" — if cheaper API-based sandboxes prove sufficient, the wedge collapses
- **OCU pricing complexity** — variable OCU consumption makes budgeting hard; finance buyers will push back
- **No laptop story at all** — they actively argue against localhost; teams that want a hybrid (local + remote) get nothing
- **Open-source ambiguity post-rebrand** — Gitpod was OSS; Ona's OSS status is much less prominent
- **VC-funded burn rate** — aggressive rebrand + leaving Kubernetes + sunsetting flagship product implies significant ongoing investment

---

## Competitive Implications for Jarvy

### Category framing
Ona/Gitpod is **adjacent**. They have explicitly bet that "the next decade of software engineering involves background agents running on remote VMs, not laptops." Jarvy bets that most developers still work on a real laptop and need that laptop provisioned reliably.

| Dimension | Ona (Gitpod) | Jarvy |
|-----------|--------------|-------|
| Category | AI agent platform + managed CDE | CLI host-environment provisioner |
| Where the env lives | Remote VM in Ona Cloud or customer VPC | Developer's laptop (macOS / Linux / Windows) |
| Config format | `devcontainer.json` + `automations.yml` | `jarvy.toml` |
| Install model | Sign up + browser, or deploy in VPC | Single binary, `jarvy setup` |
| Pricing | OCU credits ($20/mo Core entry; Enterprise custom) | OSS CLI, no infra cost |
| Buyer | Eng leader, platform team, CIO/CTO | Individual dev, team lead, platform team |
| Sales motion | Top-down enterprise sale + bottoms-up free tier for agent trials | Bottoms-up OSS adoption |
| Core argument | "Localhost is dead; agents need full VMs" | "Localhost is alive; provision it reliably" |

### Where Ona is strong vs. Jarvy
- Background agents running 24/7 (Jarvy has no agent runtime story)
- Regulated industries that already want code off the laptop
- Teams that have already committed to the CDE model
- Buyers who want OPEX (OCUs) over CAPEX (laptop fleets)
- Companies adopting AI coding agents at scale and want a governance layer

### Where Jarvy is strong vs. Ona
- **Localhost development is not dying for most teams** — Ona's own thesis is contested; Jarvy serves the majority case
- **Zero infra commitment** — `jarvy.toml` works on day 1, no VPC, no OCU budget, no platform decision
- **No vendor lock-in** — `jarvy.toml` is a plain TOML file; OCU pricing creates compounding lock-in
- **Hybrid teams** — devs who use Ona for background agents *also* need their laptops set up. Jarvy fills that gap; Ona explicitly refuses to.
- **Local AI workflows** (Claude Code, Cursor, Codex CLI on the laptop) — these are exploding and run locally. Ona is the opposite bet.
- **Predictable pricing** — free CLI vs. variable OCU consumption that scales with usage

### Opportunities
- **"Jarvy is the answer to laptops that didn't die"** — direct response content to "The last year of localhost." If Ona is wrong about localhost being dead, that's exactly Jarvy's market.
- **Hybrid stack positioning** — most teams will use *both*: Ona for long-running background agents in the cloud + Jarvy to set up the laptop they actually open every morning. There's no conflict; the marketing should make this explicit.
- **OCU-cost-curve content** — model out what an Ona deployment costs vs. a `jarvy setup` for the same outcome at small-team scale (which is most teams). "When do you actually need OCUs?"
- **Gitpod Classic refugee play** — many teams who used Gitpod Classic for plain CDE work were forced off when it sunset. Some are angry. Some are evaluating alternatives. None of those alternatives are "use your laptop again" — but for plenty of teams that's actually the right answer with Jarvy + a good `jarvy.toml`.
- **Devcontainer interop** — Ona's environments use `devcontainer.json`. If Jarvy can read a `devcontainer.json` and surface its declared tools as a Jarvy plan, teams can target one config for both worlds.
- **Trust signal**: Ona left Kubernetes. Jarvy never needed Kubernetes. That's an honest, ungilded message.

### Threats
- If "background agents on remote VMs" becomes the dominant work pattern, the entire premise of Jarvy weakens. We need to watch coding-agent adoption curves closely.
- Ona's enterprise traction (BNY, Vanta, etc.) means they have war chest + reference accounts. They can credibly tell a CIO "stop letting devs work on laptops" — that's a narrative we can't directly counter at the same buyer.
- Ona ships `llms.txt` and is winning AI-search visibility for "cloud dev environment" + "AI coding agent" queries. Jarvy needs the same surface for "developer laptop setup" / "dev environment as code (local)" terms.
- If JetBrains, VS Code, or another IDE bundles Ona-style remote sessions by default, the "real laptop" market shrinks.

---

## Raw Data Sources

- Homepage scraped: 2026-05-17 → `raw/gitpod/2026-05-17/scrapes/homepage.md`
- Pricing scraped: 2026-05-17 → `raw/gitpod/2026-05-17/scrapes/pricing.md`
- CDE/Environments page scraped: 2026-05-17 → `raw/gitpod/2026-05-17/scrapes/cde.md`
- "Ona vs. Coder" comparison post scraped: 2026-05-17 → `raw/gitpod/2026-05-17/scrapes/ona-vs-coder.md`
- "Gitpod is now Ona" rebrand announcement scraped: 2026-05-17 → `raw/gitpod/2026-05-17/scrapes/gitpod-is-now-ona.md`
- Site map (Firecrawl): 2026-05-17 — 50 URLs returned; most redirect to ona.com
- SEO data: not pulled — DataForSEO MCP not available in this environment
- Review data: not pulled in this run
