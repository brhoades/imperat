{
  description = "imperative-runner";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }: let
    pkgsFor = system: import nixpkgs {
      inherit system;
    }; in (flake-utils.lib.eachDefaultSystem (system: let
      pkgs = pkgsFor system;
    in {
      devShells.default = with pkgs; mkShell rec {
        buildInputs = [
          rustup
        ];
      };
    }));
}
