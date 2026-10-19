#!/bin/sh
# Shared environment configuration for Onyx KVM image build scripts.
# Sourced by scripts in image/ to provide consistent defaults.

CONTAINER_ENGINE=${CONTAINER_ENGINE:-podman}
ONYX_KVM_BUILDER_TAG=${ONYX_KVM_BUILDER_TAG:-localhost/onyx-kvm-builder:ubuntu-22.04-amd64}
ONYX_KVM_SOURCE_VOLUME=${ONYX_KVM_SOURCE_VOLUME:-onyx-kvm-sdk}
ONYX_KVM_WORK_VOLUME=${ONYX_KVM_WORK_VOLUME:-onyx-kvm-work}
ONYX_KVM_HOST_TOOLS_VOLUME=${ONYX_KVM_HOST_TOOLS_VOLUME:-onyx-kvm-host-tools}
TINYALSA_SOURCE_ARCHIVE=${TINYALSA_SOURCE_ARCHIVE:-/private/tmp/tinyalsa-v2.0.0.tar.gz}

export CONTAINER_ENGINE ONYX_KVM_BUILDER_TAG ONYX_KVM_SOURCE_VOLUME ONYX_KVM_WORK_VOLUME ONYX_KVM_HOST_TOOLS_VOLUME TINYALSA_SOURCE_ARCHIVE
