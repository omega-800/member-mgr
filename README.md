# member-mgr

Minimal HTTP server with a single REST endpoint to add members to a sqlite db.

## usage

Send SQL insert request formatted as `key=value` separated by `&`.

E.g.
```sh
curl localhost:1234 -X POST -H 'Authorization: Bearer <token>' -d"name=depressed&email=dead@moon.com"
```
==
```sql
INSERT INTO members (name, email) VALUES ('depressed', 'dead@moon.com');
```

If you provide `db_name` as a key, that will be used as the sqlite path.

E.g.
```sh
curl localhost:1234 -X POST -H 'Authorization: Bearer <token>' -d"db_name=vip.db&name=depressed&email=dead@moon.com"
```
will be written to `./vip.db`

## config

Configurable through env vars:

```
MEMBER_MGR_TOKEN        # Bearer auth token                   default: very-secret-token
MEMBER_MGR_ADDR         # Address to listen on                default: 0.0.0.0:1234
MEMBER_MGR_DEFAULT_DB   # DB name if not provided in request  default: members.db
MEMBER_MGR_LOG          # Set log level to info or err        default: info
MEMBER_MGR_CREATE_DB    # If members table should be created  default: yes
```

## nix

Run:
```sh
nix run github:omega-800/member-mgr
```

Inside flake:
```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    member-mgr = {
      url = "github:omega-800/member-mgr";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

## docker

Build the image:

```sh
docker build -t member-mgr .
```

Run:
```sh
docker run --rm \
  -p 1234:1234 \
  -e MEMBER_MGR_ADDR=0.0.0.0:1234 \
  -e MEMBER_MGR_TOKEN='secret' \
  member-mgr
```
