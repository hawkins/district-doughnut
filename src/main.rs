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
    AttributeValue, DynamoDb, DynamoDbClient, PutItemError, PutItemInput, PutItemOutput, ScanError,
    ScanInput, ScanOutput,
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

#[derive(PartialEq)]
struct Flavor {
    flavor: String,
    description: String,
}

fn alert(sns: &SnsClient, message: &str) -> Result<PublishResponse, PublishError> {
    match env::var("TOPIC_ARN") {
        Ok(arn) => sns
            .publish(PublishInput {
                message: String::from(message),
                topic_arn: Some(arn),
                ..Default::default()
            })
            .sync(),
        Err(_e) => {
            error!("TOPIC_ARN not set");

            // Bad practice to hijack this error type....
            Err(PublishError::NotFound(String::from("TOPIC_ARN not set")))
        }
    }
}

fn query_previous_flavors(dynamodb: &DynamoDbClient) -> Result<ScanOutput, ScanError> {
    let input = ScanInput {
        table_name: String::from("district-doughnut-flavors"),
        ..Default::default()
    };

    dynamodb.scan(input).sync()
}

fn save_new_flavor(
    dynamodb: &DynamoDbClient,
    flavor: &Flavor,
) -> Result<PutItemOutput, PutItemError> {
    let mut item = HashMap::<String, AttributeValue>::new();
    item.insert(
        String::from("flavor"),
        AttributeValue {
            s: Some(flavor.flavor.to_owned()),
            ..Default::default()
        },
    );
    item.insert(
        String::from("description"),
        AttributeValue {
            s: Some(flavor.description.to_owned()),
            ..Default::default()
        },
    );
    let input = PutItemInput {
        table_name: String::from("district-doughnut-flavors"),
        item,
        ..Default::default()
    };
    dynamodb.put_item(input).sync()
}

fn is_flavor_new(flavor: &Flavor, previous_flavors: &[Flavor]) -> bool {
    !previous_flavors.contains(flavor)
}

fn scrape_new_flavors() -> Result<Vec<Flavor>, Box<std::error::Error>> {
    let body = reqwest::get("https://www.districtdoughnut.com")?.text()?;

    let dom = Document::from(body.as_str());

    let mut flavors: Vec<Flavor> = Vec::new();
    for node in dom.find(Class("margin-wrapper").descendant(Name("a"))) {
        let flavor = node.attr("data-title").unwrap().to_owned();
        let re = Regex::new(r"<.+?>").unwrap();
        let description = re
            .replace_all(node.attr("data-description").unwrap(), "")
            .into_owned();

        flavors.push(Flavor {
            flavor,
            description,
        });
    }

    Ok(flavors)
}

fn my_handler(_e: CustomEvent, c: lambda::Context) -> Result<CustomOutput, HandlerError> {
    let sns = SnsClient::new(Region::UsEast1);
    let dynamodb = DynamoDbClient::new(Region::UsEast1);

    let mut previous_flavors: Vec<Flavor> = Vec::new();

    match query_previous_flavors(&dynamodb) {
        Ok(f) => {
            dbg!(&f);
            match f.items {
                Some(items) => {
                    for item in items {
                        // We know this is safe because both of these are String values
                        let fl: String = item["flavor"].to_owned().s.unwrap();
                        let de: String = item["description"].to_owned().s.unwrap();
                        println!("Found flavor {}", &fl);
                        previous_flavors.push(Flavor {
                            flavor: fl,
                            description: de,
                        });
                    }
                }
                None => {
                    println!("No flavors saved");
                }
            }
        }
        Err(e) => {
            println!("Error getting flavors: {}", e.to_string());
        }
    }

    match scrape_new_flavors() {
        Ok(new_flavors) => {
            let mut flavor_names = Vec::new();

            for flavor in &new_flavors {
                flavor_names.push(flavor.flavor.to_owned());

                if is_flavor_new(&flavor, &previous_flavors) {
                    let notice = format!("*NEW* {}: {}", flavor.flavor, flavor.description);
                    match alert(&sns, &notice) {
                        Ok(res) => {
                            dbg!(res);
                        }
                        Err(e) => error!("Error: {}", e.to_string()),
                    };
                    println!("{}", notice);

                    match save_new_flavor(&dynamodb, flavor) {
                        Ok(_res) => {
                            println!("Saved {} to database", flavor.flavor);
                        }
                        Err(e) => {
                            error!("Error: {}", e.to_string());
                        }
                    }
                } else {
                    println!("{}: {}", flavor.flavor, flavor.description);
                }
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
