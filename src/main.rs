extern crate regex;
extern crate reqwest;
extern crate select;

use regex::Regex;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::vec::Vec;

fn scrape() -> Result<Vec<(String, String)>, Box<std::error::Error>> {
    let body = reqwest::get("https://www.districtdoughnut.com")?.text()?;

    let dom = Document::from(body.as_str());

    let mut flavors = Vec::new();
    for node in dom.find(Class("margin-wrapper").descendant(Name("a"))) {
        let flavor = node.attr("data-title").unwrap().to_owned();
        let re = Regex::new(r"<.+?>").unwrap();
        let description = re
            .replace_all(node.attr("data-description").unwrap(), "")
            .into_owned();

        flavors.push((flavor, description));
    }

    for flavor in flavors.clone() {
        println!("{}:\t{}", flavor.0, flavor.1);
    }

    Ok(flavors)
}

fn main() {
    match scrape() {
        Ok(c) => println!("Pass: {}", c.len()),
        Err(e) => println!("Fail: {}", e.to_string()),
    }
}
