FROM ubuntu:24.04

# Prevent tzdata prompt
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt update && apt install -y \
    curl build-essential pkg-config libssl-dev \
    libudev-dev git sudo vim gnupg lsb-release ca-certificates \
    tzdata

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Solana CLI
RUN sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
ENV PATH="/root/.local/share/solana/install/active_release/bin:${PATH}"

# Install Node.js + Yarn
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt install -y nodejs && \
    npm install -g yarn

# Install AVM (Anchor Version Manager)
RUN curl -sSf https://raw.githubusercontent.com/coral-xyz/anchor/master/scripts/install-avm.sh | bash
ENV PATH="/root/.avm/bin:${PATH}"

# Use Anchor 0.31.0
RUN avm install 0.31.0 && avm use 0.31.0

# Setup workspace
WORKDIR /workspace
CMD ["bash"]
