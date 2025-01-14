{
  description = "A basic flake with a shell";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    naersk.url = "github:nix-community/naersk";
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };
  };
  outputs = {
    nixpkgs,
    flake-utils,
    naersk,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk {};
      in rec {
        # For `nix build` & `nix run`:
        defaultPackage = naersk'.buildPackage {
          src = ./.;
          # Source the environment variables from .env file before building
            buildInputs = [ pkgs.dirextras ];

            preBuild = ''
                if [ -f ./.env ]; then
                echo "Sourcing .env file for PG environment variables"
                source ./.env
                else
                echo ".env file not found!"
                fi
            '';


        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [bashInteractive cargo rustc rust-analyzer bacon cargo-watch sqlx-cli postgresql pgadmin];
          shellHook = ''
              # init the db with
              # mkdir -p ./db/
            export PGDATA="$(pwd)/db"
            export PGHOST="$(pwd)"
            export PGPORT="5432"
            if [[ ! $(grep listen_address $PGDATA/postgresql.conf) ]]; then
            echo "db does not exist, creating "
            initdb -D $PGDATA --no-locale --encoding=UTF8

            cat >> "$PGDATA/postgresql.conf" <<-EOF
            listen_addresses = 'localhost'
            port = $PGPORT
            unix_socket_directories = '$PGHOST'
            EOF

            # ...create a database using the name Postgres defaults to.
              echo "CREATE USER postgres SUPERUSER;" | postgres --single -E postgres
              echo "CREATE DATABASE postgres WITH OWNER postgres;" | postgres --single -E postgres
              fi


          '';
        };
      }
    );
}
