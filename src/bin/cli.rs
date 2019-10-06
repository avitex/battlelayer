use regex::Regex;
use std::io::{self, BufReader, BufRead};
use battlelayer::conn::ConnectionBuilder;

#[tokio::main]
async fn main() {
    let mut conn = ConnectionBuilder::new()
        .connect("109.200.214.230:25515")
        .await
        .unwrap();
    
    let input = BufReader::new(io::stdin());
    let words_regex = Regex::new(r#"(?P<word>(\\,|[^,])+)(?:,?)"#).unwrap();

    for line_res in input.lines() {
        let line = line_res.unwrap();
        let words: Vec<_> = words_regex.captures_iter(line.as_str()).map(|c| c["word"].to_string()).collect();
        let response = conn.send(words).await.unwrap();
        println!("{}", response);
    }
}
