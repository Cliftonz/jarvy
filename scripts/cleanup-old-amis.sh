#!/bin/bash
# scripts/cleanup-old-amis.sh
#
# Cleans up old Jarvy E2E AMIs, keeping only the N most recent per platform.
# This reduces storage costs from accumulated AMI snapshots.
#
# Usage:
#   ./scripts/cleanup-old-amis.sh              # Dry run (show what would be deleted)
#   ./scripts/cleanup-old-amis.sh --execute    # Actually delete old AMIs
#   ./scripts/cleanup-old-amis.sh --keep 3     # Keep 3 most recent (default: 2)
#
# Prerequisites:
#   - AWS CLI configured with appropriate permissions
#   - Permissions: ec2:DescribeImages, ec2:DeregisterImage, ec2:DeleteSnapshot

set -euo pipefail

# Configuration
PLATFORMS=("fedora-40" "arch-linux" "alpine" "freebsd-14")
AMI_NAME_PREFIX="jarvy-e2e"
KEEP_COUNT=2
DRY_RUN=true

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --execute)
            DRY_RUN=false
            shift
            ;;
        --keep)
            KEEP_COUNT="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--execute] [--keep N]"
            echo ""
            echo "Options:"
            echo "  --execute    Actually delete AMIs (default is dry run)"
            echo "  --keep N     Keep N most recent AMIs per platform (default: 2)"
            echo ""
            echo "Platforms: ${PLATFORMS[*]}"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check AWS CLI is available
if ! command -v aws &> /dev/null; then
    echo -e "${RED}Error: AWS CLI is not installed or not in PATH${NC}"
    exit 1
fi

# Check AWS credentials
if ! aws sts get-caller-identity &> /dev/null; then
    echo -e "${RED}Error: AWS credentials not configured or invalid${NC}"
    exit 1
fi

echo "========================================"
echo "Jarvy E2E AMI Cleanup"
echo "========================================"
echo "Keep count: $KEEP_COUNT most recent per platform"
if $DRY_RUN; then
    echo -e "${YELLOW}Mode: DRY RUN (use --execute to delete)${NC}"
else
    echo -e "${RED}Mode: EXECUTE (will delete AMIs!)${NC}"
fi
echo ""

TOTAL_DELETED=0
TOTAL_STORAGE_SAVED=0

for platform in "${PLATFORMS[@]}"; do
    echo "----------------------------------------"
    echo "Platform: $platform"
    echo "----------------------------------------"

    # Get AMIs sorted by creation date (newest first)
    # Format: AMI_ID TAB CreationDate TAB Name TAB SnapshotId TAB VolumeSize
    AMIS=$(aws ec2 describe-images \
        --owners self \
        --filters "Name=name,Values=${AMI_NAME_PREFIX}-${platform}-*" \
        --query 'Images[*].[ImageId,CreationDate,Name,BlockDeviceMappings[0].Ebs.SnapshotId,BlockDeviceMappings[0].Ebs.VolumeSize]' \
        --output text 2>/dev/null | sort -k2 -r || echo "")

    if [[ -z "$AMIS" ]]; then
        echo "  No AMIs found for $platform"
        continue
    fi

    # Count and list AMIs
    AMI_COUNT=$(echo "$AMIS" | wc -l | tr -d ' ')
    echo "  Found $AMI_COUNT AMIs"

    # Process each AMI
    COUNT=0
    while IFS=$'\t' read -r ami_id creation_date name snapshot_id volume_size; do
        COUNT=$((COUNT + 1))

        if [[ $COUNT -le $KEEP_COUNT ]]; then
            echo -e "  ${GREEN}KEEP${NC}: $ami_id ($name) - $creation_date"
        else
            echo -e "  ${RED}DELETE${NC}: $ami_id ($name) - $creation_date"

            if ! $DRY_RUN; then
                # Deregister AMI
                echo "    Deregistering AMI..."
                if aws ec2 deregister-image --image-id "$ami_id" 2>/dev/null; then
                    echo "    AMI deregistered"
                else
                    echo -e "    ${YELLOW}Warning: Failed to deregister AMI${NC}"
                    continue
                fi

                # Delete snapshot
                if [[ -n "$snapshot_id" && "$snapshot_id" != "None" ]]; then
                    echo "    Deleting snapshot $snapshot_id..."
                    if aws ec2 delete-snapshot --snapshot-id "$snapshot_id" 2>/dev/null; then
                        echo "    Snapshot deleted"
                        if [[ -n "$volume_size" && "$volume_size" != "None" ]]; then
                            TOTAL_STORAGE_SAVED=$((TOTAL_STORAGE_SAVED + volume_size))
                        fi
                    else
                        echo -e "    ${YELLOW}Warning: Failed to delete snapshot${NC}"
                    fi
                fi
            fi

            TOTAL_DELETED=$((TOTAL_DELETED + 1))
        fi
    done <<< "$AMIS"
done

echo ""
echo "========================================"
echo "Summary"
echo "========================================"
if $DRY_RUN; then
    echo "AMIs that would be deleted: $TOTAL_DELETED"
    echo ""
    echo -e "${YELLOW}Run with --execute to delete these AMIs${NC}"
else
    echo "AMIs deleted: $TOTAL_DELETED"
    echo "Storage freed: ~${TOTAL_STORAGE_SAVED}GB"
    echo "Estimated savings: ~\$$(echo "scale=2; $TOTAL_STORAGE_SAVED * 0.05" | bc)/month"
fi
echo ""
