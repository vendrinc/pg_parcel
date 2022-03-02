# pgslice

A solution to: https://postgrespro.com/list/thread-id/1715772

A very minimal subset of `pg_dump`, with the addition of `mysqldump`'s `--where` option. By default, all applicable tables will be dumped with `where "OrganizationId" = '19653bc3-57f4-429e-902f-bc04b0fca4dc'`.

````
pgslice 0.1.0
Jacob Elder <jacob.elder@vendr.com>
Dump horizontal slices from PostgreSQL schemas

USAGE:
    pgslice [OPTIONS]

OPTIONS:
    -c, --column <COLUMN>                [default: OrganizationId]
    -d, --database-url <DATABASE_URL>    [default: postgres://localhost:15432/postgres]
    -h, --help                           Print help information
    -i, --id <ID>                        [default: 19653bc3-57f4-429e-902f-bc04b0fca4dc]
    -s, --schema <SCHEMA>                [default: saasdash]
    -V, --version                        Print version information```
````
