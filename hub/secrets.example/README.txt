The files `compose.yaml` expects in `hub/secrets/` (git-ignored). Never commit real ones.

  db_password    the password of the Postgres user `hub`: one line of random characters
  database_url   postgres://hub:<db_password>@db:5432/hub   (one line; the password must be
                 free of characters that need URL escaping, so use hex)
  signing.key    the hub's Ed25519 seed in hex: `rustcraft-hub generate-key signing.key`
  ip-secret      at least 32 random characters keying the sign-up address hashes

README.md, "Deploying", gives the commands that create them. Back up signing.key: losing it
means every game server must be told the hub's new key.
