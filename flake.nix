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
        ];

        # Runtime / link-time libraries
        buildInputs = with pkgs; [
          duckdb   # provides libduckdb.so + duckdb.pc for duckdb-sys
        ];
      in
      {
        # -----------------------------------------------------------------
        # Development shell  —  enter with `nix develop`
        # -----------------------------------------------------------------
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          # Let duckdb-sys find the library via pkg-config
          PKG_CONFIG_PATH = "${pkgs.duckdb}/lib/pkgconfig";

          # Handy: show the binary path when entering the shell
          shellHook = ''
            echo "claudia dev shell — rust $(rustc --version)"
            echo "duckdb: $(pkg-config --modversion duckdb 2>/dev/null || echo 'check PKG_CONFIG_PATH')"
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

          PKG_CONFIG_PATH = "${pkgs.duckdb}/lib/pkgconfig";

          # Templates are embedded at compile time by askama; nothing extra needed.
          # The binary expects ANTHROPIC_API_KEY at runtime, not build time.
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
