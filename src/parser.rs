use anyhow::{Error, Result};
use indexmap::IndexMap;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, line_ending, not_line_ending, one_of},
    combinator::{map, map_res, opt},
    multi::{length_count, many0, many1},
    sequence::{delimited, separated_pair, terminated},
    IResult,
};
use serde::{Deserialize, Serialize};
use std::{str, str::FromStr};

pub fn parse(input: &str) -> Result<File, nom::Err<nom::error::Error<&str>>> {
    let (input, name) = name(input)?;
    let (input, db) = db(input)?;
    let (input, peaks) = peaks(input)?;
    assert!(input.is_empty());
    Ok(File { name, db, peaks })
}

fn name(input: &str) -> IResult<&str, String> {
    delimited(
        tag("Name:"),
        delimited(
            multiwhitespace,
            map(not_line_ending, ToString::to_string),
            multiwhitespace,
        ),
        line_ending,
    )(input)
}

fn db(input: &str) -> IResult<&str, Option<u64>> {
    opt(delimited(
        tag("DB#:"),
        delimited(multiwhitespace, number, multiwhitespace),
        line_ending,
    ))(input)
}

fn peaks(input: &str) -> IResult<&str, IndexMap<u64, u64>> {
    length_count(
        delimited(
            tag("Num Peaks:"),
            delimited(multiwhitespace, number::<usize>, multiwhitespace),
            line_ending,
        ),
        terminated(
            separated_pair(number, multiseparator, number),
            multiseparator,
        ),
    )(input)
    .map(|(input, peaks)| (input, peaks.into_iter().collect()))
}

fn multiseparator(input: &str) -> IResult<&str, Vec<String>> {
    many1(alt((
        map(one_of(" \t,;:()[]{}"), Into::into),
        map(line_ending, ToString::to_string),
    )))(input)
}

fn multiwhitespace(input: &str) -> IResult<&str, Vec<char>> {
    many0(one_of(" \t"))(input)
}

fn number<T: FromStr>(input: &str) -> IResult<&str, T> {
    map_res(digit1, str::parse)(input)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct File {
    pub name: String,
    pub db: Option<u64>,
    pub peaks: IndexMap<u64, u64>,
}

impl FromStr for File {
    type Err = Error;

    fn from_str(from: &str) -> Result<Self, Self::Err> {
        Ok(parse(from).map_err(|error| error.to_owned())?)
    }
}
