use indicatif::{ProgressBar, ProgressStyle};
use postgres::{Client, NoTls};
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

static DATABASE_URL: &str = "postgres://localhost:15432/postgres";

static SCHEMA: &str = "saasdash";

static REFLECT_QUERY: &str = r#"
    select
        table_name,
        column_name,
        case is_nullable when 'YES' then True else False end as is_nullable,
        ordinal_position
    from information_schema.columns
    where table_schema = 'saasdash'
    order by table_name, ordinal_position;
"#;

static PARTITION_COLUMN: &str = "OrganizationId";

// TODO obviously this needs to be dynamic
static PARTITION_VALUE: &str = "19653bc3-57f4-429e-902f-bc04b0fca4dc";

fn main() {
    try_main().unwrap();
}

fn try_main() -> Result<(), Box<dyn Error>> {
    let client = Client::connect(DATABASE_URL, NoTls)?;
    // The weird `COPY TO` behavior requires some acrobatics that we avoid by
    // just having a separate connection for it.
    let mut copy_client = Client::connect(DATABASE_URL, NoTls)?;

    let tables = get_tables(client)?;
    let pb = ProgressBar::new(tables.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar().template("{msg} {spinner} {wide_bar:blue} eta {eta}"),
    );
    pb.enable_steady_tick(250);

    for table in tables.iter() {
        let copy_statement = format!(
            r#"COPY ({}) TO stdout;"#,
            table.copy_out_query(PARTITION_VALUE)
        );
        pb.set_message(format!("{:>30}", table.name));
        let mut reader = copy_client.copy_out(&copy_statement)?;
        let mut buf = vec![];
        reader.read_to_end(&mut buf)?;
        println!("BEGIN;");
        println!(r#"TRUNCATE TABLE {}."{}";"#, SCHEMA, table.name);
        println!("{};", table.copy_in_query());
        println!("{}\\.", std::str::from_utf8(&buf)?);
        println!("COMMIT;");
        pb.inc(1);
    }
    pb.finish_with_message(format!("Dumped {} tables", tables.len()));

    Ok(())
}

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
}

impl Table {
    fn copy_out_query(&self, partition_column_value: &str) -> String {
        let query = match self.name.as_str() {
            "googleAuthPermissions" => {
                format!(
                    r#"
                        select {} from "googleAuthPermissions"
                        join "googleAuthTokens" on "googleAuthTokenId" = "googleAuthTokens".id
                        join "directoryUsers" on "GoogleUserId" = "directoryUsers"."externalId"
                        where "directoryUsers"."OrganizationId" = '{}'
                    "#,
                    &self.column_list_qualified(),
                    PARTITION_VALUE,
                )
            }
            "dailyExchangeRates" => {
                self.default_copy_out_query(partition_column_value) + " where True = False"
            }
            _ => self.default_copy_out_query(partition_column_value),
        };
        query
    }

    fn default_copy_out_query(&self, partition_column_value: &str) -> String {
        let mut query = format!(r#"select {} from "{}""#, &self.column_list(), &self.name);
        if let Some(org_scope) = self
            .columns
            .iter()
            .find(|column| column.name == PARTITION_COLUMN)
        {
            let mut where_clause = format!(r#""{PARTITION_COLUMN}" = '{partition_column_value}'"#);
            if org_scope.is_nullable {
                where_clause = format!(r#"({where_clause} or "{PARTITION_COLUMN}" is null)"#)
            }
            query = format!("{query} where {where_clause}");
        }
        // query = format!("{query} limit 10");
        query
    }
    fn copy_in_query(&self) -> String {
        format!(
            r#"COPY {SCHEMA}."{}" ({}) FROM stdin"#,
            self.name,
            self.column_list()
        )
    }
    fn column_list(&self) -> String {
        self.columns
            .iter()
            .map(|column| column.quoted_name())
            .collect::<Vec<String>>()
            .join(", ")
    }
    fn column_list_qualified(&self) -> String {
        self.columns
            .iter()
            .map(|column| format!(r#""{}".{}"#, self.name, column.quoted_name()))
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[derive(Debug, Clone)]
struct Column {
    pub name: String,
    pub is_nullable: bool,
    pub position: i32,
}

impl Column {
    fn quoted_name(&self) -> String {
        format!("\"{}\"", self.name)
    }
}

fn get_tables(mut client: Client) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut tables: Vec<Table> = client
        .query(REFLECT_QUERY, &[])?
        .into_iter()
        .fold(
            HashMap::new(),
            |mut acc: HashMap<String, Vec<Column>>, row| {
                let table_name: &str = row.get("table_name");
                let column = Column {
                    name: row.get("column_name"),
                    is_nullable: row.get("is_nullable"),
                    position: row.get("ordinal_position"),
                };
                if let Some(columns) = acc.get_mut(table_name) {
                    columns.push(column)
                } else {
                    acc.insert(table_name.to_owned(), vec![column]);
                }
                return acc;
            },
        )
        .into_iter()
        .map(|(name, mut columns)| {
            columns.sort_by(|a, b| a.position.cmp(&b.position));
            let table = Table { name, columns };
            table
        })
        .collect();

    tables.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tables)
}
