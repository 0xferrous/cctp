{
  description = "CCTP CLI development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            openssl
            pkg-config
            rust-analyzer
          ];

          shellHook = ''
            echo "CCTP dev shell"
            echo "Available: cargo, rustc, rustfmt, clippy, rust-analyzer"
          '';
        };
      });
}
