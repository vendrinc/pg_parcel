use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use postgres::{Client, NoTls};
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

mod sql_string;
use sql_string::SqlString;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    #[clap(default_value_t = String::from("19653bc3-57f4-429e-902f-bc04b0fca4dc"))]
    id: String,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("OrganizationId"))]
    column: String,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("saasdash"))]
    schema: String,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("postgres://localhost:15432/postgres"))]
    database_url: String,
}

fn main() {
    let args = Args::parse();
    try_main(&args).unwrap();
}

fn try_main(args: &Args) -> Result<(), Box<dyn Error>> {
    let tables = get_tables(args)?;

    let pb = ProgressBar::new(tables.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar().template("{msg} {spinner} {wide_bar:blue} eta {eta}"),
    );
    pb.enable_steady_tick(250);

    let mut client = Client::connect(&args.database_url, NoTls)?;

    for table in tables.iter() {
        let copy_statement = format!("COPY ({}) TO stdout;", table.copy_out_query(args));
        pb.set_message(format!("{:>30}", table.name));
        let mut reader = client.copy_out(&copy_statement)?;
        let mut buf = vec![];
        reader.read_to_end(&mut buf)?;
        println!("{};", table.copy_in_query());
        println!("{}\\.", std::str::from_utf8(&buf)?);
        pb.inc(1);
    }

    pb.finish_with_message(format!("Dumped {} tables", tables.len()));

    Ok(())
}
#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
    schema: String,
}

impl Table {
    fn copy_out_query(&self, args: &Args) -> String {
        let query = match self.name.as_str() {
            "googleAuthPermissions" => {
                format!(
                    r#"
                        select {} from "googleAuthPermissions"
                        join "googleAuthTokens" on "googleAuthTokenId" = "googleAuthTokens".id
                        join "directoryUsers" on "GoogleUserId" = "directoryUsers"."externalId"
                        where "directoryUsers".{} = {}
                    "#,
                    &self.column_list_qualified(),
                    args.column.sql_identifier(),
                    args.id.sql_value(),
                )
            }
            "dailyExchangeRates" => self.default_copy_out_query(args) + " where True = False",
            _ => self.default_copy_out_query(args),
        };
        query
    }

    fn default_copy_out_query(&self, args: &Args) -> String {
        let mut query = format!(
            "select {} from {}",
            &self.column_list(),
            &self.name.sql_identifier()
        );
        if let Some(org_scope) = self
            .columns
            .iter()
            .find(|column| column.name == args.column)
        {
            let mut where_clause = format!(
                "{column} = {id}",
                column = args.column.sql_identifier(),
                id = args.id.sql_value()
            );
            if org_scope.is_nullable {
                where_clause = format!(
                    "({where_clause} or {column} is null)",
                    column = args.column.sql_identifier()
                )
            }
            query = format!("{query} where {where_clause}");
        }
        // query = format!("{query} limit 10");
        query
    }
    fn copy_in_query(&self) -> String {
        format!(
            "COPY {schema}.{name} ({columns}) FROM stdin",
            schema = self.schema.sql_identifier(),
            name = self.name.sql_identifier(),
            columns = self.column_list()
        )
    }
    fn column_list(&self) -> String {
        self.columns
            .iter()
            .map(|column| column.name.sql_identifier())
            .collect::<Vec<String>>()
            .join(", ")
    }
    fn column_list_qualified(&self) -> String {
        self.columns
            .iter()
            .map(|column| format!("{}.{}", self.name, column.name.sql_identifier()))
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

fn get_tables(args: &Args) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut client = Client::connect(&args.database_url, NoTls)?;
    let query = format!(
        r#"
        select
            table_name,
            column_name,
            case is_nullable when 'YES' then True else False end as is_nullable,
            ordinal_position
        from information_schema.columns
        where table_schema = {schema}
        order by table_name, ordinal_position;
    "#,
        schema = args.schema.sql_value(),
    );
    let mut tables: Vec<Table> = client
        .query(&query, &[])?
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
            let table = Table {
                name,
                columns,
                schema: args.schema.to_owned(),
            };
            table
        })
        .collect();

    tables.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tables)
}
