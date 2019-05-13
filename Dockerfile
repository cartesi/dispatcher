FROM ubuntu:18.04

MAINTAINER Carlo Fragni <carlo@cartesi.io>

ENV DEBIAN_FRONTEND=noninteractive

ENV BASE /opt/cartesi

# Install basic development tools
# ----------------------------------------------------
RUN \
    apt-get update && \
    apt-get install --no-install-recommends -y \
        unzip build-essential autoconf automake libtool git \
        make curl g++ gcc libssl-dev pkg-config ca-certificates && \
    mkdir -p $BASE

# Install protobuf
# ----------------------------------------------------
WORKDIR $BASE
RUN \
    git clone --recurse-submodules --depth 1 https://github.com/protocolbuffers/protobuf.git

WORKDIR $BASE/protobuf
RUN \
    NPROC=$(nproc) && \
    ./autogen.sh && \
    ./configure && \
    make -j$NPROC && \
    make -j$NPROC check && \
    make -j$NPROC install && \
    ldconfig

WORKDIR $BASE
COPY . $BASE/
RUN \
    rm -rf $BASE/protobuf

# Installing a specific rust nightly version
# ----------------------------------------------------
RUN \
    curl -f -L https://static.rust-lang.org/rustup.sh -O && \
    sh rustup.sh -y --default-toolchain=nightly

#Loading cargo bin in path
ENV PATH="/root/.cargo/bin:$PATH"

#Setting a specific nightly version as default
RUN \
    rustup default nightly-2018-12-23

#Changing shell to bash and installing rust stable
#SHELL ["/bin/bash", "-c"]
#RUN \
#    curl https://sh.rustup.rs -sSf >> rust_installer.sh && \
#    chmod +x rust_installer.sh && \
#    ./rust_installer.sh -y

#Loading cargo bin in path
#ENV PATH="/root/.cargo/bin:$PATH"

# Compile emulator_interface
# ----------------------------------------------------
WORKDIR $BASE/emulator_interface
RUN \
    cargo run

#Compile compute
WORKDIR $BASE
RUN \
    cargo build --bin compute

###Cleaning up
#RUN \
#    rm -rf /var/lib/apt/lists/*

USER root
