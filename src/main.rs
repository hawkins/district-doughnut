extern crate kuchiki;
extern crate reqwest;

use kuchiki::traits::TendrilSink;

fn scrape() -> Result<(), Box<std::error::Error>> {
    let body = reqwest::get("https://www.districtdoughnut.com")?.text()?;
    //println!("{}", body);
    let selector = "div.margin-wrapper img";

    let dom = kuchiki::parse_html().one(body);

    let mut a = 0;

    for css_match in dom.select(selector).unwrap() {
        a += 1;
        let as_node = css_match.as_node();

        let flavor = as_node.text_contents();

        println!("{}", flavor);
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
