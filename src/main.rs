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
struct Item {
    item: String,
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

fn query_previous_items(dynamodb: &DynamoDbClient) -> Result<ScanOutput, ScanError> {
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

fn save_new_item(dynamodb: &DynamoDbClient, item: &Item) -> Result<PutItemOutput, PutItemError> {
    let mut table_item = HashMap::<String, AttributeValue>::new();
    table_item.insert(
        String::from("item"),
        AttributeValue {
            s: Some(item.item.to_owned()),
            ..Default::default()
        },
    );
    table_item.insert(
        String::from("description"),
        AttributeValue {
            s: Some(item.description.to_owned()),
            ..Default::default()
        },
    );
    let input = PutItemInput {
        table_name: String::from(get_table_name().unwrap()),
        item: table_item,
        ..Default::default()
    };
    dynamodb.put_item(input).sync()
}

fn remove_old_item(
    dynamodb: &DynamoDbClient,
    item: &Item,
) -> Result<DeleteItemOutput, DeleteItemError> {
    let mut key = HashMap::<String, AttributeValue>::new();
    key.insert(
        String::from("item"),
        AttributeValue {
            s: Some(item.item.to_owned()),
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

fn is_item_new(item: &Item, previous_items: &[Item]) -> bool {
    !previous_items.contains(item)
}

fn scrape_current_items() -> Result<Vec<Item>, Box<std::error::Error>> {
    let url = env::var("MENU_URL")?;
    let body = reqwest::get(&url)?.text()?;

    let dom = Document::from(body.as_str());

    let mut items: Vec<Item> = Vec::new();
    for node in dom.find(Class("product-name").descendant(Name("a"))) {
        // TODO: Make item/description scraping configurable somehow
        let item = node.text().to_owned();
        let re = Regex::new(r"<.+?>").unwrap();
        //let description = re
        //    .replace_all(node.attr("data-description").unwrap(), "")
        //    .into_owned();
        // TODO: Default description
        let description = String::from("");

        items.push(Item { item, description });
    }

    Ok(items)
}

fn my_handler(_e: CustomEvent, c: lambda::Context) -> Result<CustomOutput, HandlerError> {
    info!("Creating SNS client");
    let sns = SnsClient::new(Region::UsEast1);
    info!("Creating Dynamo client");
    let dynamodb = DynamoDbClient::new(Region::UsEast1);

    let mut previous_items: Vec<Item> = Vec::new();
    let mut current_items: Vec<Item>;
    let mut new_items: Vec<Item> = Vec::new();
    let mut item_names = Vec::new();

    info!("Querying Dynamo for previous items");
    match query_previous_items(&dynamodb) {
        Ok(f) => {
            match f.items {
                Some(items) => {
                    for item in items {
                        // We know this is safe because both of these are String values
                        let fl: String = item["item"].to_owned().s.unwrap();
                        let de: String = item["description"].to_owned().s.unwrap();
                        previous_items.push(Item {
                            item: fl,
                            description: de,
                        });
                    }
                }
                None => {
                    info!("No previous items saved");
                }
            }
        }
        Err(e) => {
            return Err(c.new_error(&format!("Error getting previous items: {}", e.to_string())));
        }
    }

    info!("Scraping website for current items");
    match scrape_current_items() {
        Ok(items) => {
            current_items = items;
        }
        Err(e) => {
            info!("Fail: {}", e.to_string());
            error!(
                "Error processing request {}: {}",
                c.aws_request_id,
                e.to_string()
            );

            return Err(c.new_error(&format!(
                "Error scraping website for new items: {}",
                e.to_string()
            )));
        }
    }

    for item in &current_items {
        item_names.push(item.item.to_owned());
        if is_item_new(&item, &previous_items) {
            new_items.push(item.clone());
        }
    }

    for item in new_items {
        let notice = format!("*NEW* {}: {}", item.item, item.description);

        info!("{}", notice);
        match alert(&sns, &notice) {
            Ok(_res) => {
                info!("Successfully notified SNS");
            }
            Err(e) => error!("Error: {}", e.to_string()),
        };

        match save_new_item(&dynamodb, &item) {
            Ok(_res) => {
                info!("Saved {} to database", item.item);
            }
            Err(e) => {
                error!("Error: {}", e.to_string());
            }
        }
    }

    let mut unavailable_items = Vec::new();
    for item in previous_items {
        if !current_items.contains(&item) {
            unavailable_items.push(item);
        }
    }
    for item in unavailable_items {
        let notice = format!("{} is no longer available", item.item);

        info!("{}", notice);

        match alert(&sns, &notice) {
            Ok(_res) => {
                info!("Successfully notified SNS");
            }
            Err(e) => error!("Error: {}", e.to_string()),
        };

        match remove_old_item(&dynamodb, &item) {
            Ok(_res) => {
                info!("Removed {} from database", item.item);
            }
            Err(e) => {
                error!("Error: {}", e.to_string());
            }
        }
    }

    info!("Done!");
    Ok(CustomOutput {
        message: format!("Found items: {}", item_names.join(", ")),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Info)?;
    lambda!(my_handler);

    Ok(())
}
