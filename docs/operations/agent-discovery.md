# Agent discovery surfaces (operations)

How jarvy.dev advertises itself to AI agents, LLM crawlers, and answer
engines — what lives in this repo, what must be configured at the
Cloudflare edge, and what is intentionally **not** published.

The site is a static MkDocs Material build served by **GitHub Pages**, with
DNS proxied through **Cloudflare** (orange cloud). GitHub Pages cannot set
custom response headers or do content negotiation, so anything header- or
negotiation-based is a Cloudflare concern.

---

## Repo-side surfaces (served by GitHub Pages)

All of these are committed under `docs/` and ship on every `main` build
(see `.github/workflows/docs.yml`). Dot-prefixed files under
`docs/.well-known/` are copied into the site by a post-build hook
(`scripts/mkdocs_hooks.py`) because MkDocs skips dot-directories.

| Surface | Source | Standard |
|---|---|---|
| `/robots.txt` | `docs/robots.txt` | Names + allows AI crawlers; declares `Content-Signal` (AIPREF) |
| `/llms.txt`, `/llms-full.txt` | repo root (symlinked into `docs/`) | [llmstxt.org](https://llmstxt.org) |
| `/sitemap.xml` | auto (MkDocs) | Sitemaps |
| `/tags.json` | `tags` plugin | page → topic index |
| `/.well-known/security.txt` | `docs/.well-known/security.txt` | RFC 9116 |
| `/.well-known/api-catalog` | `docs/.well-known/api-catalog` | RFC 9727 / RFC 9264 linkset |
| `/.well-known/agent-skills/index.json` (+ `jarvy-integration/SKILL.md`) | `docs/.well-known/agent-skills/` | [Agent Skills Discovery](https://agentskills.io) v0.2.0 |
| `<head>` JSON-LD + `<link rel>` pointers | `docs/overrides/main.html` | schema.org, RFC 8288/8631 |

**Content signals.** `docs/robots.txt` declares
`Content-Signal: search=yes, ai-input=yes, ai-train=no`. `ai-input=yes` is
the directive that welcomes live agent/RAG querying; flip `ai-train` to
`yes` to also permit model training.

**Updating the agent skill.** If you edit
`docs/.well-known/agent-skills/jarvy-integration/SKILL.md`, recompute its
digest and update `index.json`:

```bash
shasum -a 256 docs/.well-known/agent-skills/jarvy-integration/SKILL.md \
  | awk '{print "sha256:"$1}'
```

---

## Cloudflare edge configuration (NOT in this repo)

GitHub Pages can't do these. Configure them in the Cloudflare dashboard for
the `jarvy.dev` zone. They are recorded here so the config is not lost.

### 1. `Link` response headers (RFC 8288)

Rules → Transform Rules → **Modify Response Header** → Set static, header
name `Link`, value (all relations in one header, comma-separated):

```
</.well-known/api-catalog>; rel="api-catalog", </for-ai-agents/>; rel="service-doc", </schema/jarvy.schema.json>; rel="service-desc", </llms.txt>; rel="alternate"; type="text/plain"
```

The same relations exist as HTML `<link>` elements in `main.html`; the
header form is what RFC 8288 clients and `curl -I` consumers expect.

### 2. Markdown for Agents

Enable Cloudflare
[Markdown for Agents](https://developers.cloudflare.com/fundamentals/reference/markdown-for-agents/)
so requests with `Accept: text/markdown` get a markdown rendering of the
HTML page (HTML stays the browser default). Verify:

```bash
curl -H "Accept: text/markdown" -o /dev/null -w "%{content_type}\n" https://jarvy.dev/
# want: text/markdown
```

### 3. `Content-Type` for the API catalog

The well-known name is extensionless, so GitHub Pages serves
`/.well-known/api-catalog` as `application/octet-stream`. Add a Transform
Rule setting `Content-Type: application/linkset+json` for that exact path
(the JSON parses without it, but strict RFC 9727 clients check the type).

### 4. Managed robots.txt / AI bot blocking

Cloudflare's **managed robots.txt** (Security Settings → Bot traffic) and
**Block AI Scrapers and Crawlers** (Security → Bots) both operate at the
edge and will *override* / *403* the repo surfaces above if enabled:

- Managed robots.txt replaces `docs/robots.txt` with Cloudflare's version.
- "Block AI bots" returns `403` to AI user-agents, defeating agent access.

For a site whose goal is to *welcome* agent querying, either turn both off
(repo `robots.txt` wins, AI UAs get `200`) or, if you want managed signals,
keep them but confirm they agree with the repo's `Content-Signal` intent.
Test: `curl -A ClaudeBot https://jarvy.dev/llms.txt` should return `200`.

---

## Intentionally NOT published

These checklist items assume a hosted API, auth, or a remote MCP endpoint —
none of which jarvy.dev has. Publishing them would advertise capabilities
that don't exist and mislead agents. Add them only when the backing
capability is real.

| Surface | Why not (yet) |
|---|---|
| `/.well-known/openid-configuration`, `oauth-authorization-server`, `oauth-protected-resource`, `/auth.md` | No protected APIs and no agent auth/registration |
| `/.well-known/mcp/server-card.json` (SEP-1649) | Jarvy's MCP server is **local stdio** (`jarvy mcp`) — there is no hosted transport URL to advertise. Add when a remote MCP endpoint is hosted |
| WebMCP (`navigator.modelContext.provideContext`) | Static docs site — no in-page application actions worth exposing as browser tools |
| DNS-AID (SVCB/HTTPS records + DNSSEC) | Draft spec; needs a hosted agent entrypoint (A2A / remote MCP) to point records at. Revisit alongside the MCP server card |
