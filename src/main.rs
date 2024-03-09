#![warn(
    clippy::pedantic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::nursery
)]

use clap::Parser;
use main_error::MainResult;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "Convert EQ csv to YNAB csv")]
struct Args {
    #[arg(short, long)]
    filename: String,

    #[arg(short, long)]
    output: String,
}

#[derive(Debug)]
enum Err {
    InvalidNumLineElements(String),
    PrefixAmount,
    ParseAmount,
    ParsePayee,
    PrefixPayee,
    ConvertDate,
    Write(std::io::Error),
}

impl fmt::Display for Err {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidNumLineElements(s) => write!(f, "invalid number of elements: {s}"),
            Self::PrefixAmount => write!(f, "parsing prefix amount"),
            Self::ParseAmount => write!(f, "parsing amount"),
            Self::ParsePayee => write!(f, "parsing payee"),
            Self::PrefixPayee => write!(f, "removing prefix payee"),
            Self::ConvertDate => write!(f, "converting date"),
            Self::Write(err) => write!(f, "writing: {err}"),
        }
    }
}

impl Error for Err {}

#[derive(Debug)]
struct Data {
    date: String,
    payee: String,
    amount: f32,
}

fn read_file(filename: String) -> Result<String, io::Error> {
    let mut contents = String::new();
    File::open(filename)?.read_to_string(&mut contents)?;
    Ok(contents)
}

fn remove_payee_prefix(payee: &str) -> Option<&str> {
    const KEYWORDS: [&str; 3] = [" to ", " by ", " from "];

    for key in KEYWORDS {
        if payee.contains(key) {
            let split: Vec<&str> = payee.split(key).collect();
            return Some(split.last()?.trim());
        }
    }

    Some(payee)
}

fn convert_month(input_month: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];

    for (idx, month) in MONTHS.iter().enumerate() {
        if input_month.contains(month) {
            return Some(format!("{:0>2}", idx + 1));
        }
    }

    None
}

fn convert_date(date: &str) -> Option<String> {
    // 29 FEB 2024 to dd/mm/yyyy

    let split: Vec<&str> = date.split_ascii_whitespace().collect();
    if split.len() != 3 {
        return None;
    }

    let day = split.first()?;
    let year = split.last()?;
    let month = split.get(1)?;

    let month = convert_month(month)?;

    Some(format!("{day}/{month}/{year}"))
}

fn write(filename: &str, data: &[Data]) -> Result<(), std::io::Error> {
    // This shows adding to account.
    // 31/01/24,CANADA LIFE,,,,$271.8
    // This shows removing from account.
    // 29/01/24,BK OF MONTREAL,,,$610
    let mut output = String::from("Date,Payee,Catergory,Memo,Outflow,Inflow\n");

    for d in data {
        let commas = if d.amount > 0.0 { ",,,," } else { ",,," };
        output.push_str(&format!(
            "{},{}{commas}{}\n",
            d.date,
            d.payee,
            d.amount.abs()
        ));
    }

    let mut file = File::create(filename)?;
    file.write_all(output.as_bytes())?;

    Ok(())
}

fn main() -> MainResult {
    // 29 FEB 2024,Account Credited from 300605613,$1.59,$24640.45
    let args = Args::parse();
    let string: String = read_file(args.filename)?;
    // Skip first line to ignore header line.
    let lines = string.lines().skip(1);

    let data: Result<Vec<Data>, Err> = lines
        .map(|l| {
            let elements: Vec<&str> = l.split(',').collect();
            if elements.len() == 4 {
                let amount = elements
                    .get(2)
                    .ok_or(Err::InvalidNumLineElements(l.into()))?;
                // Determine if first char is '-' or '$'
                let first = amount
                    .chars()
                    .collect::<Vec<char>>()
                    .first()
                    .ok_or(Err::PrefixAmount)?
                    .to_owned();

                let is_neg = first.eq(&'-');
                let split_idx = if is_neg { 2 } else { 1 };

                let (_, amount) = amount.split_at(split_idx);
                let amount: f32 = amount.parse::<f32>().map_err(|_| Err::ParseAmount)?
                    * if is_neg { -1.0 } else { 1.0 };

                let payee = elements.get(1).ok_or(Err::ParsePayee)?;
                let payee = remove_payee_prefix(payee).ok_or(Err::PrefixPayee)?;

                let date = elements
                    .first()
                    .ok_or(Err::InvalidNumLineElements(l.into()))?;
                let date = convert_date(date).ok_or(Err::ConvertDate)?;
                Ok(Data {
                    date,
                    payee: payee.to_string(),
                    amount,
                })
            } else {
                Err(Err::InvalidNumLineElements(l.into()))
            }
        })
        .collect();
    let data = data?;

    write(&args.output, &data).map_err(Err::Write)?;

    println!("Success");
    Ok(())
}
