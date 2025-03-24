use core::panic;
use std::{
    fs::{self, File},
    io::{self, Write, stdout},
};

use serde_json::{Map, Value};

fn main() -> io::Result<()> {
    println!("generating list of missing translations at target/translations/<lang>");
    let translations_dir = fs::read_dir("public/i18n")?;
    fs::create_dir_all("target/translations")?;
    let reference: Value =
        serde_json::from_str(&fs::read_to_string("public/i18n/en/translation.json")?)
            .map_err(io::Error::other)?;
    for language in translations_dir {
        let language = language?;
        let lang_name = language
            .file_name()
            .into_string()
            .expect("should be valid unicode filename");
        if lang_name == "en" {
            continue; // skip reference lang
        }
        let mut path = language.path();
        path.push("translation.json");
        let compare: Value =
            serde_json::from_str(&fs::read_to_string(path)?).map_err(io::Error::other)?;
        let mut file = File::create(format!("target/translations/{lang_name}.txt"))?;
        println!("generate missing entries for {lang_name}");
        writeln!(
            file,
            "# This file contains an auto generated list of missing translations\n"
        )?;
        let mut count = 0;
        write_keys_not_in_reference(&mut count, String::new(), &reference, &compare, &mut file)?;
        println!();
    }

    Ok(())
}

fn write_keys_not_in_reference(
    count: &mut usize,
    parent: String,
    reference: &Value,
    compare: &Value,
    write_missing: &mut impl Write,
) -> io::Result<()> {
    let ref_map = match reference {
        Value::Object(map) => map,
        _ => panic!("invalid ref translation file"),
    };
    let map = match compare {
        Value::Null => &Map::new(),
        Value::Object(map) => map,
        _ => panic!("invalid current translation file"),
    };
    for (ref_key, ref_val) in ref_map {
        let val = if let Some(inner) = map.get(ref_key) {
            if ref_val.is_string() {
                continue; // leaf contained, skip
            }
            inner
        } else {
            if ref_val.is_string() {
                // leaf missing output
                writeln!(write_missing, "{parent}{ref_key}")?;
                *count += 1;
                print!("\r\t{count}");
                let _ = stdout().flush();
            }
            &Value::Null
        };
        if !ref_val.is_string() {
            // recurse unless were leaf
            write_keys_not_in_reference(
                count,
                format!("{parent}{ref_key}."),
                ref_val,
                val,
                write_missing,
            )?;
        }
    }
    Ok(())
}
