{
  description = "A basic flake with a shell";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.systems.url = "github:nix-systems/default";
  inputs.flake-utils = {
    url = "github:numtide/flake-utils";
    inputs.systems.follows = "systems";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
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
