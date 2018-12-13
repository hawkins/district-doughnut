extern crate regex;
extern crate reqwest;
extern crate select;

use regex::Regex;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};

fn scrape() -> Result<(), Box<std::error::Error>> {
    let body = reqwest::get("https://www.districtdoughnut.com")?.text()?;
    //println!("{}", body);

    let dom = Document::from(body.as_str());

    let mut a = 0;
    for node in dom.find(Class("margin-wrapper").descendant(Name("a"))) {
        a += 1;

        let flavor = node.attr("data-title").unwrap();
        let re = Regex::new(r"<.+?>").unwrap();
        let description = re.replace_all(node.attr("data-description").unwrap(), "");

        println!("{}", flavor);
        println!("  {}", description);
    }

    println!("Found {} flavors", a);

    Ok(())
}

fn main() {
    match scrape() {
        Ok(_) => println!("Pass"),
        Err(e) => println!("Fail: {}", e.to_string()),
    }
}
