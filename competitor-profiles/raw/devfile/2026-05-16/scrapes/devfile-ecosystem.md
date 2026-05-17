Source: https://devfile.io/docs/2.3.0/devfile-ecosystem
Scraped: 2026-05-16

# Devfile Ecosystem

Three roles:
- Devfile author / runtime provider — authors devfiles for a runtime
- Registry administrator — deploys and manages private/enterprise registries
- Application developer — consumes devfiles via supported tools

Public community registry hosted by Red Hat, managed by community.

Private registries: orgs can deploy their own internal registry and configure default registry list per cluster.

Consumer tools: tools can register catalogs of public/private registries. Each registry ships with an index server + registry viewer.

Parent devfiles: developers extend an existing parent devfile to customize a stack. Devfile can be packaged with the app source to keep consistent behavior across tools.

Disclaimer: "Tools that support the devfile spec might have varying levels of support."
