{
  description = "Enderboard: a small board of text, links and files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rust = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt ];
        # rustls uses ring, which compiles C and assembly at build time. There
        # is no C compiler in the base toolchain, so bring one in.
        ringDeps = [ pkgs.gcc pkgs.perl pkgs.nasm ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = rust ++ ringDeps ++ [
            pkgs.rust-analyzer
            pkgs.sqlite
            pkgs.nodejs_22
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };

        # What CI needs to lint and test the server, and nothing else.
        devShells.ci = pkgs.mkShell {
          buildInputs = rust ++ ringDeps;
        };

        # What CI needs to build the SPA.
        devShells.web = pkgs.mkShell {
          buildInputs = [ pkgs.nodejs_22 ];
        };
      }
    );
}
