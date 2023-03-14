![screenshot](screenshots/demo.gif)

# pg_parcel

[![CI](https://github.com/Blissfully/pg_parcel/actions/workflows/build.yml/badge.svg)](https://github.com/Blissfully/pg_parcel/actions/workflows/build.yml)
[![Release](https://github.com/Blissfully/pg_parcel/actions/workflows/release.yml/badge.svg)](https://github.com/Blissfully/pg_parcel/actions/workflows/release.yml)

A very minimal subset of `pg_dump`, but with the addition of `mysqldump`'s `--where` option. A solution to: https://postgrespro.com/list/thread-id/1715772

Most options are specified via config file.

```toml
column_name = "customer_id"
schema_name = "public"
database_url = "postgres://localhost:15432/postgres"
skip_tables = [
  "users_backup"
]

[overrides]
# We only want the one customer identified by --id on the command line
customers = """
  select * from customers where id = :id
"""
# The `user_files` table doesn't have a customer_id column, so we need to join.
user_files = """
  select user_files.*
  from users_files
  join users on users.id = user_files.user_id
  where users.customer_id = :id
"""
daily_exchange_rates = """
  select * from daily_exchange_rates
  where ARRAY['currency'] && (current_setting('pg_parcel.features')::text[]);
"""
audit_log = """
  select * from audit_log
  where user_id = :id and
  (
    ARRAY['audit'] && (current_setting('pg_parcel.features')::text[])
    or created_at >= NOW() - INTERVAL '30 days'
    or updated_at >= NOW() - INTERVAL '30 days'
  );
"""
```

## Releases

We publish binaries for both Linux x86_64 (any distro, using [musl](https://musl.libc.org/)) and macOS Universal (both Intel and Apple Silicon in a single binary).

To create a new release, update `Cargo.toml` and create a tag like `v1.2.3` (SemVer, prefixed with `v`).

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
