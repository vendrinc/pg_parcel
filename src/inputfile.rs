use serde_derive::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug)]
/// Slicefile is a way to set up a repeatable dump from a single database schema.
pub struct Slicefile {
    pub column_name: String,
    pub schema_name: String,
    pub database_url: String,
    pub skip_tables: Option<HashSet<String>>,
    pub overrides: Option<HashMap<String, String>>,
}

pub fn load(path: &Path) -> Result<Slicefile, Box<dyn Error>> {
    let slicefile_string =
        fs::read_to_string(&path).expect("Need a pg_parcel.toml file to continue");
    let slicefile: Slicefile = toml::from_str(&slicefile_string)?;
    println!("{:#?}", &slicefile);
    Ok(slicefile)
}
