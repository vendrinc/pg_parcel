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
use std::io::Write;
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
    let mut client = Client::connect(&options.database_url, NoTls)?;

    client.query("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;", &[])?;

    let tables = get_tables(&options)?;
    let mut sizes: Vec<(String, u64)> = Vec::with_capacity(tables.len());

    let pb = ProgressBar::new(tables.len() as u64);
    let pb_template = format!(
        "{{msg:>{width}.bold}} {{spinner}} {{wide_bar}} eta {{eta}} ",
        width = tables
            .iter()
            .map(|table| table.name.len())
            .max()
            .unwrap_or(30)
    );
    pb.set_style(ProgressStyle::default_bar().template(&pb_template));
    pb.enable_steady_tick(250);

    for table in tables.iter() {
        let copy_statement = format!("COPY ({}) TO stdout;", table.copy_out_query(&options));
        pb.set_message(table.name.to_owned());

        let mut stdout = std::io::stdout();
        writeln!(stdout, "{};", table.copy_in_query())?;
        let mut reader = client.copy_out(&copy_statement)?;
        let size = std::io::copy(&mut reader, &mut stdout)?;
        sizes.push((table.name.clone(), size));
        writeln!(stdout, "\\.")?;

        pb.inc(1);
    }
    pb.finish_with_message(format!("Dumped {} tables", tables.len()));

    client.query("ROLLBACK", &[])?;

    // Summarize table sizes.
    {
        let total = sizes.iter().map(|(.., size)| *size).sum::<u64>();
        if total > 0 {
            eprintln!("       Bytes | % of total | Table name");
            sizes.sort_by_key(|(.., size)| *size);
            for (name, size) in sizes.iter() {
                let percent = ((*size as f64) * 100f64) / (total as f64);
                eprintln!("{size:12} | {percent:9.1}% | {name}");
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
    schema: String,
    size: u64, // Bytes.
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
            "select {} from {}.{}",
            &self.column_list(),
            &self.schema.sql_identifier(),
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
          tables.table_name,
          pg_total_relation_size(tables.table_schema || '.' || tables.table_name)::text as table_size,
          array_agg(columns.column_name::text order by columns.ordinal_position) as table_column_names,
          array_agg(columns.is_nullable = 'YES' order by columns.ordinal_position) as table_columns_nullable
        from information_schema.tables
        join information_schema.columns on (
          columns.table_catalog = tables.table_catalog
          and columns.table_schema = tables.table_schema
          and columns.table_name = tables.table_name)
        where tables.table_schema = {schema}
        and tables.table_type = 'BASE TABLE'
        group by tables.table_schema, tables.table_name
        order by tables.table_schema, tables.table_name
        "#,
        schema = options.schema.sql_value(),
    );
    let mut tables: Vec<Table> = client
        .query(&query, &[])?
        .into_iter()
        .filter_map(|row| {
            let table_name: String = row.get("table_name");
            if !options.skip_tables.contains(&table_name) {
                let table_size_s: String = row.get("table_size");
                let table_size: u64 = table_size_s.parse().unwrap_or(0);
                let table_column_names: Vec<&str> = row.get("table_column_names");
                let table_column_nullables: Vec<bool> = row.get("table_columns_nullable");
                let table_columns = (1..)
                    .zip(table_column_names)
                    .zip(table_column_nullables)
                    .map(|((position, name), is_nullable)| Column {
                        name: name.to_owned(),
                        is_nullable,
                        position,
                    })
                    .collect();
                Some(Table {
                    name: table_name,
                    columns: table_columns,
                    schema: options.schema.clone(),
                    size: table_size,
                })
            } else {
                None
            }
        })
        .collect();

    tables.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tables)
}
