# Variables for EC2 Self-Hosted Runner Module

variable "platform" {
  description = "Platform identifier (e.g., fedora-40, arch-linux, alpine, freebsd-14)"
  type        = string
}

variable "ami_id" {
  description = "AMI ID for the runner instance"
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type (t3.small recommended for most, t3.micro for Alpine)"
  type        = string
  default     = "t3.small"
}

variable "vpc_id" {
  description = "VPC ID where the runner will be deployed"
  type        = string
}

variable "subnet_id" {
  description = "Subnet ID for the runner instance"
  type        = string
}

variable "github_repo" {
  description = "GitHub repository in owner/repo format"
  type        = string
}

variable "runner_labels" {
  description = "Labels for the GitHub runner (comma-separated)"
  type        = string
}

variable "spot_max_price" {
  description = "Maximum hourly price for Spot instance (empty = on-demand price)"
  type        = string
  default     = ""
}

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default = {
    Project   = "jarvy"
    Component = "e2e-testing"
    ManagedBy = "terraform"
  }
}
