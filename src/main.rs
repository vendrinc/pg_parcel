mod inputfile;
mod sql_string;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use inputfile::InputFile;
use lazy_static::lazy_static;
use postgres::{Client, NoTls};
use regex::Regex;
use sql_string::SqlString;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io::Read;
use std::path::Path;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// Command line arguments
struct Args {
    /// Dump only columns where column_name is this value
    #[clap(short, long)]
    id: String,

    /// Tables with this column name will only include rows with the value specified by <ID>
    #[clap(short, long)]
    column_name: Option<String>,

    #[clap(short, long)]
    #[clap(default_value_t = String::from("./pg_parcel.toml"))]
    file: String,
}

/// Options here is a combination of command line arguments and contents of the slicefile.
struct Options {
    column_name: String,
    column_value: String,
    schema: String,
    database_url: String,
    skip_tables: HashSet<String>,
    overrides: HashMap<String, String>,
}

impl Options {
    pub fn load() -> Result<Options, Box<dyn Error>> {
        let args = Args::parse();
        let file = InputFile::load(Path::new(&args.file))?;
        let options = Options {
            column_name: if let Some(column_name) = args.column_name {
                column_name
            } else {
                file.column_name
            },
            column_value: args.id,
            database_url: file.database_url,
            schema: file.schema_name,
            skip_tables: file.skip_tables.unwrap_or_default(),
            overrides: file.overrides.unwrap_or_default(),
        };
        Ok(options)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::load()?;
    let tables = get_tables(&options)?;

    let pb = ProgressBar::new(tables.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar().template("{msg:>30.bold} {spinner} {wide_bar} eta {eta}"),
    );
    pb.enable_steady_tick(250);

    let mut client = Client::connect(&options.database_url, NoTls)?;

    for table in tables.iter() {
        let copy_statement = format!("COPY ({}) TO stdout;", table.copy_out_query(&options));
        pb.set_message(table.name.to_owned());
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
    fn copy_out_query(&self, options: &Options) -> String {
        if let Some(query) = options.overrides.get(&self.name) {
            lazy_static! {
                static ref RE: Regex = Regex::new(":id").unwrap();
            }
            let query = RE
                .replace_all(query, &options.column_value.sql_value())
                .to_string();
            return query;
        }
        let mut query = format!(
            "select {} from {}",
            &self.column_list(),
            &self.name.sql_identifier()
        );
        if let Some(org_scope) = self
            .columns
            .iter()
            .find(|column| column.name == options.column_name)
        {
            let mut where_clause = format!(
                "{column} = {id}",
                column = options.column_name.sql_identifier(),
                id = options.column_value.sql_value()
            );
            if org_scope.is_nullable {
                where_clause = format!(
                    "({where_clause} or {column} is null)",
                    column = options.column_name.sql_identifier()
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
}

#[derive(Debug, Clone)]
struct Column {
    pub name: String,
    pub is_nullable: bool,
    pub position: i32,
}

fn get_tables(options: &Options) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut client = Client::connect(&options.database_url, NoTls)?;
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
        schema = options.schema.sql_value(),
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
                acc
            },
        )
        .into_iter()
        .filter(|(name, ..)| !options.skip_tables.contains(name))
        .map(|(name, mut columns)| {
            columns.sort_by(|a, b| a.position.cmp(&b.position));
            Table {
                name,
                columns,
                schema: options.schema.to_owned(),
            }
        })
        .collect();

    tables.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tables)
}
