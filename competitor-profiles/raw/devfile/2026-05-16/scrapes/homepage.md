Source: https://devfile.io/
Scraped: 2026-05-16

# Devfile.io homepage

Tagline: "Simplify and accelerate your workflow."
Subhead: "An open standard defining containerized development environments."

CTAs: Get started (docs), View on GitHub

Example yaml shown on hero:
```yaml
schemaVersion: 2.2.0
metadata:
  name: go
  language: go
components:
  - container:
      endpoints:
        - name: http
          targetPort: 8080
      image: quay.io/devfile/golang:latest
      memoryLimit: 1024Mi
      mountSources: true
    name: runtime
```

Section: Develop Faster — "Take control of your development environment. Devfiles defines best practices for your application lifecycle."

Pillars:
- Reproducible — environments quick to create, throwaway, easily re-created
- Consistent — share configs across projects, single source of truth
- Secure — central location management, updates applied once
- Community — share expertise from other developers and communities

Key Features:
- Stacks & Starter Projects
- Community Registry
- Custom Registry
- Parent Support

Footer note: "We are a Cloud Native Computing Foundation sandbox project."

Nav: Registry (registry.devfile.io), Docs, Get Started, GitHub (devfile/api), Slack (kubernetes.slack.com #devfile).

Meta keywords: Devfile, OpenShift, Kubernetes
