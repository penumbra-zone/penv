{
  description = "A nix development shell and build environment for penv, the Penumbra environment manager";

  inputs = {
    # nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, ... }:
    let
      # Read the application version from the local `Cargo.toml` file.
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = cargoToml.package.version;
    in
    {
      # Export the version so it's accessible outside the build context
      inherit version;
    } // (flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs { inherit system; };
          # Permit version declarations, but default to unset,
          # meaning the local working copy will be used.
          penvRelease = null;

          # Set up for Rust builds.
          craneLib = crane.mkLib pkgs;
          # Important environment variables so that the build can find the necessary libraries
          LIBCLANG_PATH="${pkgs.libclang.lib}/lib";
          ROCKSDB_LIB_DIR="${pkgs.rocksdb.out}/lib";

        in with pkgs; with pkgs.lib; let
          # Build the `penv` binary
          penv = (craneLib.buildPackage {
            pname = "penv";
            # what
            src = cleanSourceWith {
              src = if penvRelease == null then craneLib.path ./. else fetchFromGitHub {
                owner = "penumbra-zone";
                repo = "penv";
                rev = "v${penvRelease.version}";
                sha256 = "${penvRelease.sha256}";
              };
              filter = path: type:
                # Retain non-rust files as build inputs, for shell configs
                (builtins.match ".*\.j2$" path != null) ||
                # ... as well as all the normal cargo source files:
                (craneLib.filterCargoSources path type);
            };
            nativeBuildInputs = [ pkg-config ];
            buildInputs = [
              clang openssl
              ] ++ lib.optionals pkgs.stdenv.isDarwin [
                # mac-only deps
                pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
                pkgs.darwin.apple_sdk.frameworks.CoreServices
            ];

            inherit system LIBCLANG_PATH ROCKSDB_LIB_DIR;

            meta = {
              description = "An environment manager for Penumbra tooling";
              homepage = "https://github.com/penumbra-zone/penv";
              license = [ licenses.mit licenses.asl20 ];
            };
          }).overrideAttrs (_: { doCheck = false; }); # Disable tests to improve build times

        in {
          packages = {
            inherit penv ;
            default = penv;
          };
          apps = {
            penv.type = "app";
            penv.program = "${penv}/bin/penv";
          };
          devShells.default = craneLib.devShell {
            inputsFrom = [ penv ];
            packages = [
              cargo-nextest
              cargo-release
              cargo-watch
              just
              nix-prefetch-scripts
              sqlfluff
            ];
          };
        }
      )
    );
}
