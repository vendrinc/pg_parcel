# pg_parcel

A solution to: https://postgrespro.com/list/thread-id/1715772

It's like a very minimal subset `pg_dump`, but with the addition of `mysqldump`'s `--where` option.

```
pg_parcel 0.1.3
Jacob Elder <jacob.elder@vendr.com>
Dump horizontal slices from PostgreSQL schemas

USAGE:
    pg_parcel [OPTIONS] --id <ID>

OPTIONS:
    -f, --file <FILE>    [default: ./pg_parcel.toml]
    -h, --help           Print help information
    -i, --id <ID>
    -V, --version        Print version information
```

Example `pg_parcel.toml` file:

```toml
column_name = "customer_id"
schema_name = "public"
database_url = "postgres://localhost:15432/postgres"
skip_tables = [
  "daily_exchange_rates"
]

[overrides]
# The `user_files` table doesn't have a customer_id column, so we need to join.
user_files = """
  select user_files.*
  from users_files
  join users on users.id = user_files.user_id
  where users.customer_id = :id
"""
```
