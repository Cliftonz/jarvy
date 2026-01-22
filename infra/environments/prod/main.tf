# Production Environment for Jarvy E2E Testing
#
# This configuration creates the necessary infrastructure for
# running E2E tests on EC2 Spot instances.

terraform {
  required_version = ">= 1.5"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  # Uncomment to use remote state
  # backend "s3" {
  #   bucket = "jarvy-terraform-state"
  #   key    = "e2e-testing/prod/terraform.tfstate"
  #   region = "us-west-2"
  # }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "jarvy"
      Environment = "prod"
      Component   = "e2e-testing"
      ManagedBy   = "terraform"
    }
  }
}

# Variables
variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-west-2"
}

variable "github_repo" {
  description = "GitHub repository (owner/repo)"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID for runner instances"
  type        = string
}

variable "subnet_id" {
  description = "Subnet ID for runner instances"
  type        = string
}

# AMI IDs (populated after building with Packer)
variable "fedora_ami_id" {
  description = "AMI ID for Fedora 40"
  type        = string
  default     = ""
}

variable "arch_ami_id" {
  description = "AMI ID for Arch Linux"
  type        = string
  default     = ""
}

variable "alpine_ami_id" {
  description = "AMI ID for Alpine"
  type        = string
  default     = ""
}

variable "freebsd_ami_id" {
  description = "AMI ID for FreeBSD 14"
  type        = string
  default     = ""
}

# EC2 Runner Modules
#
# Instance sizing rationale:
# - t3.small (2 vCPU, 2GB RAM): Fedora, Arch, FreeBSD - sufficient for Rust compilation
# - t3.micro (1 vCPU, 1GB RAM): Alpine only - minimal OS, lighter builds
#
# Spot pricing (us-west-2, ~20 min job):
# - t3.small: ~$0.005/run
# - t3.micro: ~$0.002/run

module "fedora_runner" {
  source = "../../modules/ec2-runner"
  count  = var.fedora_ami_id != "" ? 1 : 0

  platform      = "fedora-40"
  ami_id        = var.fedora_ami_id
  instance_type = "t3.small"  # 2 vCPU, 2GB RAM
  vpc_id        = var.vpc_id
  subnet_id     = var.subnet_id
  github_repo   = var.github_repo
  runner_labels = "self-hosted,self-hosted-fedora,linux,x64"
}

module "arch_runner" {
  source = "../../modules/ec2-runner"
  count  = var.arch_ami_id != "" ? 1 : 0

  platform      = "arch-linux"
  ami_id        = var.arch_ami_id
  instance_type = "t3.small"  # 2 vCPU, 2GB RAM
  vpc_id        = var.vpc_id
  subnet_id     = var.subnet_id
  github_repo   = var.github_repo
  runner_labels = "self-hosted,self-hosted-arch,linux,x64"
}

module "alpine_runner" {
  source = "../../modules/ec2-runner"
  count  = var.alpine_ami_id != "" ? 1 : 0

  platform      = "alpine"
  ami_id        = var.alpine_ami_id
  instance_type = "t3.micro"  # 1 vCPU, 1GB RAM - Alpine is lightweight
  vpc_id        = var.vpc_id
  subnet_id     = var.subnet_id
  github_repo   = var.github_repo
  runner_labels = "self-hosted,self-hosted-alpine,linux,x64"
}

module "freebsd_runner" {
  source = "../../modules/ec2-runner"
  count  = var.freebsd_ami_id != "" ? 1 : 0

  platform      = "freebsd-14"
  ami_id        = var.freebsd_ami_id
  instance_type = "t3.small"  # 2 vCPU, 2GB RAM
  vpc_id        = var.vpc_id
  subnet_id     = var.subnet_id
  github_repo   = var.github_repo
  runner_labels = "self-hosted,self-hosted-freebsd,freebsd,x64"
}

# Outputs
output "fedora_launch_template_id" {
  value = length(module.fedora_runner) > 0 ? module.fedora_runner[0].launch_template_id : null
}

output "arch_launch_template_id" {
  value = length(module.arch_runner) > 0 ? module.arch_runner[0].launch_template_id : null
}

output "alpine_launch_template_id" {
  value = length(module.alpine_runner) > 0 ? module.alpine_runner[0].launch_template_id : null
}

output "freebsd_launch_template_id" {
  value = length(module.freebsd_runner) > 0 ? module.freebsd_runner[0].launch_template_id : null
}
