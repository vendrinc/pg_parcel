use serde_derive::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process;
use suggest::Suggest;

#[derive(Deserialize, Debug)]
pub struct InputFile {
    pub column_name: String,
    pub schema_name: String,
    pub database_url: Option<String>,
    pub accept_invalid_certs: Option<bool>,
    pub skip_tables: Option<HashSet<String>>,
    pub overrides: Option<HashMap<String, String>>,
    pub features: Option<HashSet<String>>,
}

impl InputFile {
    pub fn load(path: &Path) -> Result<InputFile, Box<dyn Error>> {
        let string = fs::read_to_string(path).expect("Need a pg_parcel.toml file to continue");
        let inputfile: InputFile = toml::from_str(&string)?;
        Ok(inputfile)
    }

    // Sanity check requested features against the configured features
    pub fn validate_features(&self, args: &[String]) {
        match &self.features {
            None => {
                eprintln!("To use --features, define some in pg_parcel.toml first!");
                process::exit(1)
            }
            Some(defined) => {
                for arg in args.iter() {
                    if !defined.contains(arg) {
                        if let Some(sugg) = defined.suggest(arg) {
                            eprintln!("Did you mean `{}`?", sugg);
                        }
                        eprintln!("No feature named `{arg}` defined in input file");
                        process::exit(1);
                    }
                }
            }
        }
    }
}
