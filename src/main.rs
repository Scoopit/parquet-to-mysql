use std::{fs::File, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{bail, Context, Result};
use itertools::Itertools;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet_to_mysql::record_batch_to_sql_inserts;

mod convert;

#[derive(Parser)]
struct Opts {
    /// SQL Table name, if not present, filename without the extension will be used instead
    #[arg(short, long)]
    table_name: Option<String>,

    /// Number of rows to group into a single INSERT INTO statement
    #[arg(short = 'r', long, default_value = "100")]
    rows_batch_size: usize,

    input_file: String,
}

const HEADER: &str = r#"/*!40101 SET @OLD_CHARACTER_SET_CLIENT=@@CHARACTER_SET_CLIENT */;
/*!40101 SET @OLD_CHARACTER_SET_RESULTS=@@CHARACTER_SET_RESULTS */;
/*!40101 SET @OLD_COLLATION_CONNECTION=@@COLLATION_CONNECTION */;
/*!50503 SET NAMES utf8mb4 */;
/*!40103 SET @OLD_TIME_ZONE=@@TIME_ZONE */;
/*!40103 SET TIME_ZONE='+00:00' */;
/*!40101 SET @OLD_SQL_MODE=@@SQL_MODE, SQL_MODE='NO_AUTO_VALUE_ON_ZERO' */;"#;

const FOOTER: &str = r#"/*!40103 SET TIME_ZONE=@OLD_TIME_ZONE */;
/*!40101 SET SQL_MODE=@OLD_SQL_MODE */;
/*!40101 SET CHARACTER_SET_CLIENT=@OLD_CHARACTER_SET_CLIENT */;
/*!40101 SET CHARACTER_SET_RESULTS=@OLD_CHARACTER_SET_RESULTS */;
/*!40101 SET COLLATION_CONNECTION=@OLD_COLLATION_CONNECTION */;"#;

fn main() -> Result<()> {
    color_eyre::install()?;
    let opts = Opts::parse();

    if opts.rows_batch_size == 0 {
        bail!("rows-batch-size must be greater than 0");
    }

    let input_path = PathBuf::from(&opts.input_file);
    let extension = input_path
        .extension()
        .expect("output filename must have an extension")
        .to_str()
        .unwrap();
    let filename = input_path
        .file_name()
        .unwrap()
        .to_str()
        .map(|f| &f[0..(f.len() - extension.len() - 1)])
        .unwrap();

    let table_name = opts
        .table_name
        .clone()
        .unwrap_or_else(|| filename.to_string());

    let file = File::open(input_path)
        .with_context(|| format!("Unable to open file {}", opts.input_file))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).context("Invalid parquet file")?;

    let columns_names = builder
        .schema()
        .fields()
        .iter()
        .map(|field| format!("`{}`", field.name()))
        .join(",");

    let mut reader = builder.build().context("Unable to build parquet reader")?;

    println!("{HEADER}");

    while let Some(batch) = reader.next() {
        let batch = batch?;

        println!(
            "{}",
            record_batch_to_sql_inserts(
                batch,
                &table_name,
                Some(columns_names.as_str()),
                opts.rows_batch_size
            )
        );
    }

    println!("{FOOTER}");

    Ok(())
}
