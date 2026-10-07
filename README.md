# member-mgr

Minimal HTTP server with a single REST endpoint to add members to a sqlite db.

## usage

Send "SQL insert request" formatted as `application/x-www-form-urlencoded`.

### simple example

```sh
curl http://localhost:1234/submit \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data "name=igrok&email=dead@moon.com"
```
Will be executed as
```sql
INSERT INTO members (name, email) VALUES ('igrok', 'dead@moon.com');
```

### custom db

If you provide `db_name` as a key, that will be used as the sqlite path.

E.g.
```sh
curl http://localhost:1234/submit \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data "db_name=vip.db&name=igrok&email=dead@moon.com"
```
will be written to `./vip.db`

### auth

Set `MEMBER_MGR_TOKEN=<your-token>` and `MEMBER_MGR_USE_TOKEN=yes` to use Bearer Auth.

```sh
curl http://localhost:1234/submit \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -H 'Authorization: Bearer <token>' \
  --data "name=igrok&email=dead@moon.com"
```

### altcha

Set `MEMBER_MGR_ALTCHA_HMAC_SECRET` and `MEMBER_MGR_ALTCHA_HMAC_KEY_SECRET` to use altcha PoW. Set the altcha token in the `altchaToken` field.

```sh
curl http://localhost:1234/challenge 
# solve your challenge
curl http://localhost:1234/submit \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data "altchaToken=<your-token>&name=igrok&email=dead@moon.com"
```

## mail

With the `mail` feature, a mail is sent to the `email` field of every submission. Sending is best-effort: if it fails, the member is still saved and the error is logged.

Configure through `MAIL_CFG_<NAME>` env vars. `<NAME>` is either `default` or a recipient address (lowercased) to override the default config for specific recipients. The value is a `;`-separated list of `key=value` pairs:

```sh
MAIL_CFG_DEFAULT='from=you@example.com;subject=Welcome {{name}};body_path=./welcome.txt;smtp_username=you@example.com;smtp_password=<app-password>'
```

| key             | description                                        | default        |
| --------------- | -------------------------------------------------- | -------------- |
| `from`          | sender address (required)                          |                |
| `from_name`     | sender display name                                |                |
| `subject`       | subject line (required)                            |                |
| `body_path`     | path to the mail body file (required)              |                |
| `smtp_host`     | SMTP host                                          | smtp.gmail.com |
| `smtp_port`     | SMTP port                                          | 465            |
| `smtp_username`       | SMTP username, set together with `smtp_password`       |                |
| `smtp_password`       | SMTP password, set together with `smtp_username`       |                |
| `additional_receiver` | fixed recipient of an additional mail, set together with `additional_subject` and `additional_body_path` |                |
| `additional_subject`  | subject line of the additional mail                    |                |
| `additional_body_path`| path to the additional mail's body file                |                |

`subject` and the body file support `{{placeholder}}` substitution with the submitted fields, e.g. `{{name}}` or `{{email}}`.

### additional mail

Setting `additional_receiver`, `additional_subject`, and `additional_body_path` together adds a second mail that is always sent to the same fixed recipient (e.g. an administrative notification), using the same sender and SMTP settings as the primary mail:

```sh
MAIL_CFG_DEFAULT='from=you@example.com;subject=Welcome {{name}};body_path=./welcome.txt;additional_receiver=admin@example.com;additional_subject=New member: {{name}};additional_body_path=./admin.txt'
```

## config

Configurable through env vars:

```sh
MEMBER_MGR_TOKEN        # Bearer auth token                   default: very-secret-token
MEMBER_MGR_USE_TOKEN    # If Bearer auth should be used       default: no
MEMBER_MGR_ADDR         # Address to listen on                default: 0.0.0.0:1234
MEMBER_MGR_DEFAULT_DB   # DB name if not provided in request  default: members.db
MEMBER_MGR_LOG          # Set log level to info or err        default: info
MEMBER_MGR_CREATE_DB    # If members table should be created  default: yes

MEMBER_MGR_ALTCHA_HMAC_SECRET
MEMBER_MGR_ALTCHA_HMAC_KEY_SECRET

MAIL_CFG_DEFAULT        # mail config, see above
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

### features

- altcha
- mail
- dotenvy
