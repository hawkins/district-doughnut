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
    AttributeValue, DeleteItemError, DeleteItemInput, DeleteItemOutput, DynamoDb, DynamoDbClient,
    PutItemError, PutItemInput, PutItemOutput, ScanError, ScanInput, ScanOutput,
};
use rusoto_sns::{PublishError, PublishInput, PublishResponse, Sns, SnsClient};
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::borrow::ToOwned;
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

#[derive(PartialEq, Clone)]
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
        table_name: String::from(get_table_name().unwrap()),
        ..Default::default()
    };

    dynamodb.scan(input).sync()
}

fn get_table_name() -> Option<String> {
    match env::var("TABLE_NAME") {
        Ok(table_name) => Some(table_name),
        Err(_e) => {
            error!("TABLE_NAME not set");
            None
        }
    }
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
        table_name: String::from(get_table_name().unwrap()),
        item,
        ..Default::default()
    };
    dynamodb.put_item(input).sync()
}

fn remove_old_flavor(
    dynamodb: &DynamoDbClient,
    flavor: &Flavor,
) -> Result<DeleteItemOutput, DeleteItemError> {
    let mut key = HashMap::<String, AttributeValue>::new();
    key.insert(
        String::from("flavor"),
        AttributeValue {
            s: Some(flavor.flavor.to_owned()),
            ..Default::default()
        },
    );
    let input = DeleteItemInput {
        table_name: String::from(get_table_name().unwrap()),
        key,
        ..Default::default()
    };
    dynamodb.delete_item(input).sync()
}

fn is_flavor_new(flavor: &Flavor, previous_flavors: &[Flavor]) -> bool {
    !previous_flavors.contains(flavor)
}

fn scrape_current_flavors() -> Result<Vec<Flavor>, Box<std::error::Error>> {
    let body = reqwest::get("https://www.districtdoughnut.com/doughnuts")?.text()?;

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
    info!("Creating SNS client");
    let sns = SnsClient::new(Region::UsEast1);
    info!("Creating Dynamo client");
    let dynamodb = DynamoDbClient::new(Region::UsEast1);

    let mut previous_flavors: Vec<Flavor> = Vec::new();
    let mut current_flavors: Vec<Flavor> = Vec::new();
    let mut new_flavors: Vec<Flavor> = Vec::new();
    let mut flavor_names = Vec::new();

    info!("Querying Dynamo for previous flavors");
    match query_previous_flavors(&dynamodb) {
        Ok(f) => {
            match f.items {
                Some(items) => {
                    for item in items {
                        // We know this is safe because both of these are String values
                        let fl: String = item["flavor"].to_owned().s.unwrap();
                        let de: String = item["description"].to_owned().s.unwrap();
                        previous_flavors.push(Flavor {
                            flavor: fl,
                            description: de,
                        });
                    }
                }
                None => {
                    info!("No previous flavors saved");
                }
            }
        }
        Err(e) => {
            return Err(c.new_error(&format!(
                "Error getting previous flavors: {}",
                e.to_string()
            )));
        }
    }

    info!("Scraping website for current flavors");
    match scrape_current_flavors() {
        Ok(flavors) => {
            current_flavors = flavors;
        }
        Err(e) => {
            info!("Fail: {}", e.to_string());
            error!(
                "Error processing request {}: {}",
                c.aws_request_id,
                e.to_string()
            );

            return Err(c.new_error(&format!(
                "Error scraping website for new flavors: {}",
                e.to_string()
            )));
        }
    }

    for flavor in &current_flavors {
        flavor_names.push(flavor.flavor.to_owned());
        if is_flavor_new(&flavor, &previous_flavors) {
            new_flavors.push(flavor.clone());
        }
    }

    for flavor in new_flavors {
        let notice = format!("*NEW* {}: {}", flavor.flavor, flavor.description);

        info!("{}", notice);
        match alert(&sns, &notice) {
            Ok(_res) => {
                info!("Successfully notified SNS");
            }
            Err(e) => error!("Error: {}", e.to_string()),
        };

        match save_new_flavor(&dynamodb, &flavor) {
            Ok(_res) => {
                info!("Saved {} to database", flavor.flavor);
            }
            Err(e) => {
                error!("Error: {}", e.to_string());
            }
        }
    }

    let mut unavailable_flavors = Vec::new();
    for flavor in previous_flavors {
        if !current_flavors.contains(&flavor) {
            unavailable_flavors.push(flavor);
        }
    }
    for flavor in unavailable_flavors {
        let notice = format!("{} is no longer available", flavor.flavor);

        info!("{}", notice);

        match alert(&sns, &notice) {
            Ok(_res) => {
                info!("Successfully notified SNS");
            }
            Err(e) => error!("Error: {}", e.to_string()),
        };

        match remove_old_flavor(&dynamodb, &flavor) {
            Ok(_res) => {
                info!("Removed {} from database", flavor.flavor);
            }
            Err(e) => {
                error!("Error: {}", e.to_string());
            }
        }
    }

    info!("Done!");
    Ok(CustomOutput {
        message: format!("Found flavors: {}", flavor_names.join(", ")),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Info)?;
    lambda!(my_handler);

    Ok(())
}
