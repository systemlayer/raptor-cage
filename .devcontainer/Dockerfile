ARG VARIANT
FROM mcr.microsoft.com/devcontainers/rust:${VARIANT}
RUN rm /etc/sudoers.d/vscode

RUN apt-get update &&\
    export DEBIAN_FRONTEND=noninteractive &&\
    apt-get install -y neovim tmux &&\
    apt-get clean &&\
    rm -rf /var/lib/apt/lists/*

# Temporarily switch user to install utilities.
USER vscode
RUN cargo install cargo-audit cargo-edit
RUN printf 'set -g mouse on\n' > ~/.tmux.conf &&\
    printf '\nPS1="${PS1::-2}\\n$ "\nexport EDITOR=nvim\n' >> ~/.bashrc
# Go back to the default Dockerfile user (can be checked with "whoami" during
# build), terminal and other processes will still run as vscode.
USER root

ENV CODEX_HOME=/opt/codex
RUN install -d -o vscode -g vscode /opt/codex &&\
    install -d /usr/local/share/codex

ARG CODEX_URL
ARG CODEX_SHA256
RUN curl --fail --location --output /usr/local/share/codex/install.sh "${CODEX_URL}" &&\
    echo "${CODEX_SHA256}  /usr/local/share/codex/install.sh" | sha256sum --check - &&\
    chmod +x /usr/local/share/codex/install.sh

# Codex runs inside Docker and it's restricted to the workspace, using bwrap
# would make setup way more complex for little to no benefit.
RUN printf '%s\n' \
        'sandbox_mode = "danger-full-access"' \
        'approval_policy = "on-request"' \
        '[features]' \
        'memories = false' \
        'daemon_auto_start = false' \
        '[memories]' \
        'generate_memories = false' \
        'use_memories = false' \
        '[analytics]' \
        'enabled = false' \
        > /usr/local/share/codex/config.toml
