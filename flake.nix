{
  description = "claudia — Rust/Axum web interface to the Claude API";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay    = {
      url    = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs     = import nixpkgs { inherit system overlays; };

        # Pin to the stable toolchain declared in rust-toolchain.toml (if
        # present), otherwise fall back to latest stable.
        rustToolchain = pkgs.rust-bin.stable.latest.default;

        # Native build inputs required to compile / link the project
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          gcc
          go-task
        ];

        # Runtime / link-time libraries (duckdb is now statically linked via bundled feature)
        buildInputs = with pkgs; [
          openssl  # still needed by reqwest for anything not rustls; harmless to include
        ];
      in
      {
        # -----------------------------------------------------------------
        # Development shell  —  enter with `nix develop`
        # -----------------------------------------------------------------
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          shellHook = ''
            echo "claudia dev shell — rust $(rustc --version)"
            echo "duckdb: bundled (static)"
          '';
        };

        # -----------------------------------------------------------------
        # Nix package build  —  `nix build`
        # -----------------------------------------------------------------
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname   = "claudia";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src     = pkgs.lib.cleanSource ./.;

          cargoLock.lockFile = ./Cargo.lock;

          inherit nativeBuildInputs buildInputs;

          # Templates are embedded at compile time by askama; nothing extra needed.
          # The binary expects ANTHROPIC_API_KEY at runtime, not build time.
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
