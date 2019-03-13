#[macro_use]
extern crate lambda_runtime as lambda;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate regex;
extern crate reqwest;
extern crate rusoto_core;
extern crate rusoto_dynamodb;
extern crate select;
extern crate simple_logger;

use lambda::error::HandlerError;
use regex::Regex;
use rusoto_core::Region;
use rusoto_dynamodb::{
    AttributeValue, DynamoDb, DynamoDbClient, GetItemInput, ScanError, ScanInput, ScanOutput,
};
use rusoto_sns::{PublishError, PublishInput, PublishResponse, Sns, SnsClient};
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::vec::Vec;

#[derive(Deserialize)]
struct CustomEvent {}

#[derive(Serialize, Clone)]
struct CustomOutput {
    message: String,
}

fn alert(message: &str) -> Result<PublishResponse, PublishError> {
    match env::var("TOPIC_ARN") {
        Ok(arn) => {
            let client = SnsClient::new(Region::UsEast1);
            client
                .publish(PublishInput {
                    message: String::from(message),
                    topic_arn: Some(arn),
                    ..Default::default()
                })
                .sync()
        }
        Err(_e) => {
            error!("TOPIC_ARN not set");

            // Bad practice to hijack this error type....
            Err(PublishError::NotFound(String::from("TOPIC_ARN not set")))
        }
    }
}

fn get_flavors() -> Result<ScanOutput, ScanError> {
    let input = ScanInput {
        table_name: String::from("district-doughnut-flavors"),
        ..Default::default()
    };

    let client = DynamoDbClient::new(Region::UsEast1);
    client.scan(input).sync()
}

fn is_flavor_new(flavor: &str) -> bool {
    let mut query_key: HashMap<String, AttributeValue> = HashMap::new();
    query_key.insert(
        String::from("flavor"),
        AttributeValue {
            s: Some(flavor.to_string()),
            ..Default::default()
        },
    );

    let query_flavors = GetItemInput {
        key: query_key,
        table_name: String::from("district-doughnut-flavors"),
        ..Default::default()
    };

    let client = DynamoDbClient::new(Region::UsEast1);

    match client.get_item(query_flavors).sync() {
        Ok(result) => result.item.is_none(),
        Err(error) => {
            panic!("Error: {:?}", error);
        }
    }
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
        if is_flavor_new(&flavor.0) {
            let notice = format!("*NEW* {}: {}", flavor.0, flavor.1);
            match alert(&notice) {
                Ok(res) => {
                    dbg!(res);
                }
                Err(e) => error!("Error: {}", e.to_string()),
            };
            println!("{}", notice);
        } else {
            println!("{}: {}", flavor.0, flavor.1);
        }
    }

    Ok(flavors)
}

fn my_handler(_e: CustomEvent, c: lambda::Context) -> Result<CustomOutput, HandlerError> {
    match get_flavors() {
        Ok(flavors) => {
            dbg!(flavors);
        }
        Err(e) => {
            println!("Error getting flavors: {}", e.to_string());
        }
    }

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
