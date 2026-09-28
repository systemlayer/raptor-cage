ARG VARIANT
FROM mcr.microsoft.com/devcontainers/rust:${VARIANT}

RUN apt-get update &&\
    export DEBIAN_FRONTEND=noninteractive &&\
    apt-get install -y cloc &&\
    apt-get clean &&\
    rm -rf /var/lib/apt/lists/*
RUN rm /etc/sudoers.d/vscode

# Temporarily switch user to install utilities.
USER vscode
RUN cargo install cargo-audit cargo-edit

ARG CODEX_URL
ARG CODEX_SHA256
RUN codex_installer="$(mktemp)" &&\
    curl --fail --location --output "${codex_installer}" "${CODEX_URL}" &&\
    echo "${CODEX_SHA256}  ${codex_installer}" | sha256sum --check - &&\
    chmod +x "${codex_installer}" &&\
    "${codex_installer}" &&\
    rm "${codex_installer}"

# Codex runs inside Docker and it's restricted to the workspace, using bwrap
# would make setup way more complex for little to no benefit.
RUN mkdir -p ~/.codex &&\
    printf '%s\n' \
        'sandbox_mode = "danger-full-access"' \
        'approval_policy = "on-request"' \
        '[features]' \
        'memories = false' \
        '[memories]' \
        'generate_memories = false' \
        'use_memories = false' \
        '[analytics]' \
        'enabled = false' \
        > ~/.codex/config.toml

# Go back to the default Dockerfile user (can be checked with "whoami" during
# build), terminal and other processes will still run as vscode.
USER root
