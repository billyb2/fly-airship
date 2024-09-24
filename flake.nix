{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };

  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain =
          pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource (craneLib.path ./.);
        buildInputs = with pkgs;
          [ mold-wrapped ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          buildInputs = buildInputs;
        };
        cargoArtifactsProd = craneLib.buildDepsOnly {
          inherit src;
          buildInputs = buildInputs;
        };

        my-crate = craneLib.buildPackage {
          inherit cargoArtifacts src;
          cargoVendorDir =
            craneLib.vendorCargoDeps { cargoLock = ./Cargo.lock; };

          buildInputs = buildInputs;
        };

        my-crate-prod = craneLib.buildPackage {
          inherit cargoArtifactsProd src;
          cargoVendorDir =
            craneLib.vendorCargoDeps { cargoLock = ./Cargo.lock; };
          buildInputs = buildInputs;
        };
      in {
        packages.default = my-crate;
        packages.build-prod = my-crate-prod;
        packages.build-deps-prod = cargoArtifactsProd;

        devShells.default = craneLib.devShell {
          inputsFrom = [ my-crate ];

          packages = with pkgs; [
            cargo-outdated
            cargo-watch
            protolint
            sqlx-cli
            rust-analyzer
          ];
        };
      });
}
