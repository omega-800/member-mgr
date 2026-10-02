{
  description = "rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      systems = nixpkgs.lib.platforms.unix;
      eachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              config = { };
              overlays = [
                rust-overlay.overlays.default
                self.overlays.default
              ];
            }
          )
        );
      pname = "member-mgr";
    in
    {
      overlays.default = _: prev: {
        rustToolchain = prev.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rustfmt"
          ];
        };
      };
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShellNoCC {
          packages = with pkgs; [
            rustToolchain
            rust-analyzer
            openssl
            pkg-config
            sqlite
          ];
          env = {
            RUST_BACKTRACE = "full";
            RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          };
        };
      });
      packages = eachSystem (
        pkgs:
        let
          fs = pkgs.lib.fileset;
          root = ./.;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            inherit pname;
            version = "0.1.0";
            src = fs.toSource {
              inherit root;
              fileset = fs.intersection (fs.gitTracked root) (
                fs.unions [
                  ./Cargo.toml
                  ./Cargo.lock
                  (fs.fileFilter (f: f.hasExt "rs") ./src)
                ]
              );
            };
            cargoLock.lockFile = ./Cargo.lock;
          };
        }
      );
      apps = eachSystem (
        pkgs:
        pkgs.lib.mapAttrs (_: drv: {
          type = "app";
          program = "${drv}${drv.passthru.exePath or "/bin/${drv.pname or drv.name}"}";
        }) self.packages.${pkgs.stdenv.hostPlatform.system}
      );
    };
}
