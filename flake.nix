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
            x86_64-linux = "0rz3jlwqfnk05avhyiss72mgv4r8862js7ka2x8kz49fdlgdzfd2";
            aarch64-linux = "1franjddiw8j4fdzp4ff899xx6s7dajrd1i0647xvzr1kdl2c42h";
            x86_64-darwin = "0lp280xgzv20w9zm7bjfvp9hm4bs6iq88zirn9b6hvqvga6vgc6w";
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

      });
}
