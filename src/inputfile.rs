use serde_derive::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct InputFile {
    pub column_name: String,
    pub schema_name: String,
    pub database_url: Option<String>,
    pub accept_invalid_certs: Option<bool>,
    pub skip_tables: Option<HashSet<String>>,
    pub overrides: Option<HashMap<String, String>>,
    pub default_features: Option<HashSet<String>>,
}

impl InputFile {
    pub fn load(path: &Path) -> Result<InputFile, Box<dyn Error>> {
        let string = fs::read_to_string(path).expect("Need a pg_parcel.toml file to continue");
        let inputfile: InputFile = toml::from_str(&string)?;
        Ok(inputfile)
    }
}
