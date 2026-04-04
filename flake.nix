{
  description = "CCTP CLI development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachSystem [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
    ] (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;

        svmReleasesList = pkgs.fetchurl {
          url = {
            x86_64-linux = "https://binaries.soliditylang.org/linux-amd64/list.json";
            aarch64-linux = "https://binaries.soliditylang.org/linux-arm64/list.json";
            x86_64-darwin = "https://binaries.soliditylang.org/macosx-amd64/list.json";
          }.${system};
          sha256 = {
            x86_64-linux = "1labmjyg3vpyjr2q6idhy3wzsj92wxj3qvf32h0g2b8nsnhf0z1g";
            aarch64-linux = "15j5p9iz51npy2jjfaqkwwd4gbkapnry8jzq41yv1y7qwg1z0d8v";
            x86_64-darwin = "0ika5973adqvbw5nk5h46wjymwzr3sq92vahzcmvrw6adif1v8wz";
          }.${system};
        };

        commonArgs = {
          pname = "cctp";
          version = "0.1.0";
          src = ./.;
          strictDeps = true;
          SVM_RELEASES_LIST_JSON = svmReleasesList;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            openssl
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        cctp = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        packages = {
          inherit cctp;
          default = cctp;
        };

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
