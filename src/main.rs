#[macro_use]
extern crate lambda_runtime as lambda;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate regex;
extern crate reqwest;
extern crate select;
extern crate simple_logger;

use lambda::error::HandlerError;
use regex::Regex;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::error::Error;
use std::vec::Vec;

#[derive(Deserialize)]
struct CustomEvent {}

#[derive(Serialize, Clone)]
struct CustomOutput {
    message: String,
}

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

fn my_handler(_e: CustomEvent, c: lambda::Context) -> Result<CustomOutput, HandlerError> {
    match scrape() {
        Ok(flavors) => {
            let mut flavor_names = Vec::new();
            for flavor in flavors {
                flavor_names.push(flavor.0);
            }

            Ok(CustomOutput {
                message: format!("Found flavors: {}", flavor_names.join(", ")),
            })
        }
        Err(e) => {
            println!("Fail: {}", e.to_string());
            error!(
                "Error processing request {}: {}",
                c.aws_request_id,
                e.to_string()
            );
            Err(c.new_error("Error scraping website"))
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Info)?;
    lambda!(my_handler);

    Ok(())
}
