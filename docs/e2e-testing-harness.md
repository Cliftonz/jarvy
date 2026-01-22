# E2E Cross-Platform Testing Harness

This document describes Jarvy's end-to-end testing infrastructure for validating tool installations across all supported platforms.

## Overview

The E2E testing harness is a **pre-release validation gate** that uses a hybrid approach to maximize platform coverage while minimizing costs:

- **GitHub-hosted runners** (free): macOS, Ubuntu, Windows
- **AWS EC2 Spot instances** (~$2-3/month): Fedora, Arch, Alpine, FreeBSD

E2E tests run **only before releases** (on release creation or manual trigger), not on every PR or merge. This validates that Jarvy correctly installs a base set of tools on real systems with real package managers before shipping.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GitHub Actions Workflow                              │
│                    (PR, merge to main, scheduled, manual)                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
            ┌─────────────────────────┼─────────────────────────┐
            ▼                         ▼                         ▼
┌───────────────────────┐ ┌───────────────────────┐ ┌───────────────────────┐
│  GitHub-Hosted Jobs   │ │  GitHub-Hosted Jobs   │ │  Self-Hosted Jobs     │
│  (macOS - FREE)       │ │  (Ubuntu/Win - FREE)  │ │  (AWS EC2 Spot)       │
│                       │ │                       │ │                       │
│  • macos-13 (Intel)   │ │  • ubuntu-22.04       │ │  • fedora-40          │
│  • macos-14 (ARM M1)  │ │  • ubuntu-24.04       │ │  • arch-linux         │
│                       │ │  • windows-latest     │ │  • alpine             │
│                       │ │                       │ │  • freebsd-14         │
└───────────────────────┘ └───────────────────────┘ └───────────────────────┘
            │                         │                         │
            └─────────────────────────┼─────────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Results Aggregation Job                               │
│                                                                              │
│  • Collects results from all matrix jobs                                    │
│  • Posts PR comment with platform × tool status matrix                      │
│  • Uploads artifacts to GitHub                                              │
│  • Sets final commit status (success/failure)                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Platform Coverage

| Platform | Runner | Type | Instance | Cost | Notes |
|----------|--------|------|----------|------|-------|
| macOS Intel | `macos-13` | GitHub-hosted | - | Free | x86_64 architecture |
| macOS ARM | `macos-14` | GitHub-hosted | - | Free | Apple Silicon M1 |
| Ubuntu 22.04 | `ubuntu-22.04` | GitHub-hosted | - | Free | LTS with apt |
| Ubuntu 24.04 | `ubuntu-24.04` | GitHub-hosted | - | Free | Latest LTS |
| Windows | `windows-latest` | GitHub-hosted | - | Free | Windows Server 2022 |
| Fedora 40 | `self-hosted-fedora` | AWS EC2 Spot | t3.small | ~$0.005/run | dnf package manager |
| Arch Linux | `self-hosted-arch` | AWS EC2 Spot | t3.small | ~$0.005/run | pacman package manager |
| Alpine | `self-hosted-alpine` | AWS EC2 Spot | t3.micro | ~$0.002/run | apk, lightweight |
| FreeBSD 14 | `self-hosted-freebsd` | AWS EC2 Spot | t3.small | ~$0.005/run | pkg package manager |

**Instance sizing rationale:**
- `t3.micro` (1 vCPU, 1GB RAM): Alpine only - minimal OS, fast builds
- `t3.small` (2 vCPU, 2GB RAM): Fedora, Arch, FreeBSD - sufficient for Rust compilation

## Tool Test Tiers

The harness tests a curated set of tools that cover all installation patterns:

### Tier 1: Core Tools (Required on All Platforms)
| Tool | Why Selected |
|------|-------------|
| `git` | Universal prerequisite, tests all package managers |
| `jq` | Simple install, uniform across distros |
| `ripgrep` | Rust binary, tests cargo fallback |
| `curl` | Network utility, version detection |
| `wget` | Tests package naming differences |

### Tier 2: Language Runtimes
| Tool | Why Selected |
|------|-------------|
| `node` | Most requested, tests version_manager support |
| `python` | Tests pyenv integration |
| `go` | Simple binary, GOPATH setup |

### Tier 3: DevOps Tools
| Tool | Why Selected |
|------|-------------|
| `kubectl` | Tests flexible dependencies |
| `terraform` | Binary install pattern |

### Tier 4: Dependency Validation
| Tool | Why Selected |
|------|-------------|
| `lazygit` | No dependencies (baseline) |

## Setup Requirements

### Prerequisites

1. **GitHub Repository** with Actions enabled
2. **AWS Account** (for EC2 Spot runners)
3. **Terraform** >= 1.0 installed locally
4. **Packer** >= 1.8 installed locally
5. **AWS CLI** configured with appropriate credentials

### GitHub Setup

No special configuration needed for GitHub-hosted runners. They work out of the box.

### AWS Setup

#### 1. Configure AWS Credentials

Create an IAM user or role with the following permissions:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "ec2:RunInstances",
        "ec2:TerminateInstances",
        "ec2:DescribeInstances",
        "ec2:CreateTags",
        "ec2:DescribeImages",
        "ec2:DescribeSecurityGroups",
        "ec2:CreateSecurityGroup",
        "ec2:AuthorizeSecurityGroupIngress",
        "ec2:AuthorizeSecurityGroupEgress"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "ssm:GetParameter",
        "ssm:PutParameter"
      ],
      "Resource": "arn:aws:ssm:*:*:parameter/jarvy/*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "iam:PassRole"
      ],
      "Resource": "arn:aws:iam::*:role/jarvy-ec2-runner-role"
    }
  ]
}
```

#### 2. Store GitHub Runner Token

Create an SSM parameter for the GitHub runner registration token:

```bash
aws ssm put-parameter \
  --name "/jarvy/github-runner-token" \
  --type "SecureString" \
  --value "YOUR_GITHUB_RUNNER_TOKEN"
```

To get a runner token:
1. Go to your repository Settings → Actions → Runners
2. Click "New self-hosted runner"
3. Copy the token from the configuration command

#### 3. Build Custom AMIs

The EC2 runners use custom AMIs with Rust and the GitHub Actions runner pre-installed.

```bash
cd infra/packer

# Build all AMIs
packer build fedora-40.pkr.hcl
packer build arch-linux.pkr.hcl
packer build alpine.pkr.hcl
packer build freebsd-14.pkr.hcl
```

Each AMI includes:
- Base OS packages and development tools
- Rust toolchain (via rustup)
- GitHub Actions runner in `/opt/actions-runner`
- Startup script that registers as ephemeral runner

#### 4. Deploy Infrastructure

```bash
cd infra/environments/prod

# Initialize Terraform
terraform init

# Review the plan
terraform plan

# Apply the infrastructure
terraform apply
```

This creates:
- Security groups for EC2 runners
- IAM roles for runner instances
- Launch templates for each platform

#### 5. Add GitHub Secrets

Add these secrets to your GitHub repository (Settings → Secrets and variables → Actions):

| Secret | Description |
|--------|-------------|
| `AWS_ACCESS_KEY_ID` | AWS access key for EC2 operations |
| `AWS_SECRET_ACCESS_KEY` | AWS secret key |
| `AWS_REGION` | AWS region (e.g., `us-east-1`) |

## Usage

### Pre-Release Validation

E2E tests are designed as a **pre-release gate**, not continuous integration. They run:

| Trigger | Platforms | When |
|---------|-----------|------|
| Release creation | All platforms | When a release/pre-release is created |
| Manual dispatch | Configurable | On demand before releases |

**Why pre-release only?**
- **Cost efficiency**: EC2 Spot costs are minimal but add up with frequent runs
- **Speed**: Full 9-platform suite takes ~20 minutes
- **Purpose**: Validates release candidates, not every code change
- **CI coverage**: Unit tests and integration tests run on every PR

### Triggering Before a Release

**Option 1: Manual trigger before tagging**

1. Go to Actions → E2E Cross-Platform Tests
2. Click "Run workflow"
3. Select the branch/commit to validate
4. Wait for all platforms to pass
5. Create the release tag

**Option 2: Automatic on release creation**

The workflow triggers automatically when you:
- Create a GitHub Release (draft or published)
- Push a tag matching `v*` (e.g., `v1.2.3`, `v1.2.3-rc.1`)

```bash
# Create a pre-release tag to trigger E2E
git tag v1.2.3-rc.1
git push origin v1.2.3-rc.1
```

### Manual Trigger Options

When running manually, you can configure:

| Option | Description | Default |
|--------|-------------|---------|
| Branch | Branch or tag to test | `main` |
| Include EC2 runners | Run Fedora/Arch/Alpine/FreeBSD | `true` |
| Tools to test | Comma-separated list or "all" | `all` |
| Skip GitHub-hosted | Only run EC2 platforms | `false` |

### Optional: Enabling Nightly Runs

If you want regression detection between releases, uncomment the schedule trigger in the workflow:

```yaml
on:
  # Uncomment to enable nightly runs
  # schedule:
  #   - cron: '0 2 * * *'  # 2 AM UTC daily
  release:
    types: [created]
  push:
    tags:
      - 'v*'
  workflow_dispatch:
```

**Note**: Nightly runs will increase monthly costs by ~$3-4 (30 runs × ~$0.10/run).

### Viewing Results

#### PR Comments

For pull requests, a comment is automatically posted with the test matrix:

```
## E2E Test Results

| Platform | git | jq | ripgrep | node | Status |
|----------|-----|-----|---------|------|--------|
| macOS Intel | ✅ | ✅ | ✅ | ✅ | Passed |
| macOS ARM | ✅ | ✅ | ✅ | ✅ | Passed |
| Ubuntu 22.04 | ✅ | ✅ | ✅ | ✅ | Passed |
| Windows | ✅ | ✅ | ✅ | ❌ | Failed |
```

#### Artifacts

Each platform uploads artifacts containing:

| File | Contents |
|------|----------|
| `results.json` | Structured test results |
| `jarvy-output.log` | Full stdout/stderr |
| `system-info.txt` | OS version, arch, package manager versions |

Download artifacts from the workflow run page.

### Debugging Failures

1. **Check the workflow logs** for the failing job
2. **Download artifacts** for detailed output
3. **Review system-info.txt** for environment details
4. **Check jarvy-output.log** for the actual error

For EC2 runner failures:
- Instances auto-terminate, so you can't SSH in after the fact
- Re-run the job with debug logging enabled
- Check CloudWatch logs if configured

## Cost Management

### Monthly Cost Estimate (Pre-Release Only)

With E2E tests running only for releases (~2-4 releases/month) and small instances:

| Component | Usage | Cost |
|-----------|-------|------|
| GitHub-hosted runners | Unlimited (public repos) | **$0** |
| EC2 Spot t3.small (Fedora/Arch/FreeBSD) | ~10 runs × $0.005 | ~$0.05 |
| EC2 Spot t3.micro (Alpine) | ~10 runs × $0.002 | ~$0.02 |
| AMI storage (4 × 8GB) | ~32GB | ~$1.60 |
| Data transfer | ~1GB/month | ~$0.09 |
| **Total** | | **~$1.75/month** |

**With optional nightly runs enabled**: Add ~$2/month (30 runs × ~$0.07/run)

### Cost Optimization Tips

1. **Pre-release only**: E2E tests run only for releases, not PRs or merges
2. **Small instances**: t3.small for most distros, t3.micro for Alpine
3. **Clean up old AMIs**: Delete previous versions after validating new ones (see below)
4. **Set cost alerts**: Configure AWS Budgets for $5/month threshold
5. **Batch releases**: Combine changes into fewer releases when possible

### AMI Cleanup

Old AMIs accumulate storage costs (~$0.05/GB/month). Delete them after validating new versions.

**Manual cleanup:**

```bash
# List all Jarvy E2E AMIs
aws ec2 describe-images \
  --owners self \
  --filters "Name=name,Values=jarvy-e2e-*" \
  --query 'Images[*].[ImageId,Name,CreationDate]' \
  --output table

# Delete old AMI (replace ami-xxx with actual ID)
aws ec2 deregister-image --image-id ami-xxx

# Delete associated snapshot
aws ec2 delete-snapshot --snapshot-id snap-xxx
```

**Automated cleanup script:**

```bash
#!/bin/bash
# scripts/cleanup-old-amis.sh
# Keeps only the 2 most recent AMIs per platform

PLATFORMS=("fedora-40" "arch-linux" "alpine" "freebsd-14")

for platform in "${PLATFORMS[@]}"; do
  echo "Cleaning up old AMIs for $platform..."

  # Get AMIs sorted by creation date (newest first)
  AMIS=$(aws ec2 describe-images \
    --owners self \
    --filters "Name=name,Values=jarvy-e2e-$platform-*" \
    --query 'Images | sort_by(@, &CreationDate) | reverse(@) | [*].ImageId' \
    --output text)

  # Skip first 2 (keep them), delete the rest
  COUNT=0
  for ami in $AMIS; do
    COUNT=$((COUNT + 1))
    if [ $COUNT -gt 2 ]; then
      echo "  Deleting $ami..."

      # Get snapshot ID
      SNAPSHOT=$(aws ec2 describe-images \
        --image-ids "$ami" \
        --query 'Images[0].BlockDeviceMappings[0].Ebs.SnapshotId' \
        --output text)

      # Deregister AMI
      aws ec2 deregister-image --image-id "$ami"

      # Delete snapshot
      if [ "$SNAPSHOT" != "None" ]; then
        aws ec2 delete-snapshot --snapshot-id "$SNAPSHOT"
      fi
    fi
  done
done

echo "Cleanup complete."
```

**Run after each AMI rebuild:**

```bash
# Build new AMIs
cd infra/packer && packer build .

# Validate new AMIs work (run E2E manually)
# Then clean up old ones
./scripts/cleanup-old-amis.sh
```

### Setting Up Cost Alerts

```bash
aws budgets create-budget \
  --account-id YOUR_ACCOUNT_ID \
  --budget '{
    "BudgetName": "jarvy-e2e-testing",
    "BudgetLimit": {"Amount": "10", "Unit": "USD"},
    "TimeUnit": "MONTHLY",
    "BudgetType": "COST"
  }' \
  --notifications-with-subscribers '[{
    "Notification": {
      "NotificationType": "ACTUAL",
      "ComparisonOperator": "GREATER_THAN",
      "Threshold": 80
    },
    "Subscribers": [{
      "SubscriptionType": "EMAIL",
      "Address": "your-email@example.com"
    }]
  }]'
```

## Security Considerations

### Ephemeral Runners

EC2 instances are **ephemeral**:
- Spin up for a single job
- Auto-terminate after completion
- No persistent state or secrets

### IAM Least Privilege

Runner instances have minimal permissions:
- Can only register/deregister as GitHub runner
- Can only read SSM parameters under `/jarvy/`
- Cannot access other AWS resources

### Network Isolation

EC2 runners are in a restricted security group:
- Outbound: HTTPS (443) for GitHub API and package mirrors
- Outbound: SSH (22) disabled by default
- Inbound: None (no SSH access)

### No Secrets on Runners

- GitHub Actions secrets are never passed to EC2 instances
- Runner registration token is the only sensitive data
- Token is fetched from SSM at boot, then deleted

## Troubleshooting

### Common Issues

#### EC2 Runner Not Registering

**Symptoms**: Job stuck waiting for runner

**Causes**:
1. Runner token expired (tokens last 1 hour)
2. IAM permissions incorrect
3. Security group blocking outbound HTTPS

**Solutions**:
1. Refresh the token in SSM Parameter Store
2. Verify IAM role has `ssm:GetParameter` permission
3. Check security group allows 443 outbound

#### Spot Instance Interrupted

**Symptoms**: Job fails with "Runner lost connection"

**Causes**: AWS reclaimed the Spot instance due to capacity

**Solutions**:
1. Job will be marked as "interrupted", not failed
2. Re-run the job (different instance will be used)
3. Consider using on-demand fallback for critical tests

#### Package Installation Failures

**Symptoms**: Tool installation fails on specific platform

**Causes**:
1. Package name differs on this distro
2. Package manager mirror is down
3. Dependency not available

**Solutions**:
1. Check Jarvy's tool definition for this platform
2. Try a different mirror in jarvy.toml
3. Add missing dependency to the tool's `depends_on`

### Logs and Debugging

#### Enable Debug Logging

Add to workflow dispatch inputs:
```yaml
env:
  JARVY_LOG: debug
  RUST_BACKTRACE: 1
```

#### View CloudWatch Logs (if configured)

```bash
aws logs get-log-events \
  --log-group-name /jarvy/e2e-runners \
  --log-stream-name fedora-40-$(date +%Y%m%d)
```

## Maintenance

### Monthly Tasks

1. **Rebuild AMIs**: Run Packer builds to get latest OS updates
2. **Clean up old AMIs**: Run `./scripts/cleanup-old-amis.sh` after validating new builds
3. **Review costs**: Check AWS Cost Explorer (should be <$2/month)
4. **Update runner version**: Bump GitHub Actions runner in Packer templates

### After Each AMI Rebuild

```bash
# 1. Build new AMIs
cd infra/packer
packer build fedora-40.pkr.hcl
packer build arch-linux.pkr.hcl
packer build alpine.pkr.hcl
packer build freebsd-14.pkr.hcl

# 2. Run E2E manually to validate
gh workflow run e2e-cross-platform.yml

# 3. If successful, clean up old AMIs
./scripts/cleanup-old-amis.sh
```

### Quarterly Tasks

1. **Review tool coverage**: Add/remove tools based on user feedback
2. **Update OS versions**: Add new LTS releases, deprecate old ones
3. **Security audit**: Review IAM permissions and security groups
4. **Instance size review**: Check if smaller instances are viable

## File Reference

```
infra/
├── README.md                          # Infrastructure overview
├── modules/
│   └── ec2-runner/
│       ├── main.tf                    # EC2 Spot + SG + IAM
│       ├── variables.tf               # Module inputs (instance sizes)
│       ├── outputs.tf                 # Module outputs
│       └── user-data.sh               # Runner bootstrap script
├── packer/
│   ├── fedora-40.pkr.hcl             # Fedora AMI template
│   ├── arch-linux.pkr.hcl            # Arch Linux AMI template
│   ├── alpine.pkr.hcl                # Alpine AMI template (t3.micro)
│   └── freebsd-14.pkr.hcl            # FreeBSD AMI template
└── environments/
    └── prod/
        ├── main.tf                    # Production config
        └── terraform.tfvars           # Environment variables

scripts/
└── cleanup-old-amis.sh                # Delete old AMI versions

tests/
├── e2e_base_tools.rs                  # E2E integration test
└── fixtures/
    └── e2e-base-tools.toml            # Test fixture

.github/workflows/
└── e2e-cross-platform.yml             # GitHub Actions workflow

docs/
└── e2e-testing-harness.md             # This documentation
```

## Related Documentation

- [PRD-038: Hybrid Cross-Platform E2E Testing Harness](../prd/038-aws-ec2-e2e-testing-harness.md)
- [Task Tracking: PRD-038](../tasks/prd-038-e2e-testing-harness.json)
- [CLAUDE.md](../CLAUDE.md) - Build commands and architecture overview
