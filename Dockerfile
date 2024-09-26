FROM nixos/nix:latest as builder

COPY flake.nix flake.lock /tmp/build/
COPY rust-toolchain.toml /tmp/build/
COPY Cargo.toml Cargo.lock /tmp/build/
COPY .cargo/ /tmp/build/
WORKDIR /tmp/build

COPY server /tmp/build/server
COPY client /tmp/build/client

RUN nix --extra-experimental-features "nix-command flakes" --option filter-syscalls false build '.#server'

FROM scratch
WORKDIR /app
COPY --from=builder /tmp/build/result/bin/ ./bin/

CMD ["/app/bin/fly-airship-server"]
