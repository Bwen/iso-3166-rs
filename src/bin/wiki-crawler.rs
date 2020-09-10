extern crate iso_3166;
const URL_WIKI_ISO_3166_1: &str = "https://en.wikipedia.org/wiki/ISO_3166-1";
const URL_WIKI_ISO_3166_2: &str = "https://en.wikipedia.org/wiki/ISO_3166-2";
const URL_WIKI_COMMONS: &str = "https://upload.wikimedia.org/wikipedia/commons";
const FILE_DATA_COUNTRY: &str = "data/countries.csv";
const FILE_LAST_MOD_COUNTRY: &str = "data/countries_lastmod";
const FILE_DATA_TERRITORY: &str = "data/territories.csv";
const FILE_LAST_MOD_TERRITORY: &str = "data/territories_lastmod";
const DIR_DATA_COUNTRY_FLAGS: &str = "data/images/country_flags";
const DIR_DATA_TERRITORY_FLAGS: &str = "data/images/territory_flags";

use scraper::{Html, Selector, ElementRef};
use std::{fs, io};
use std::fs::File;
use std::io::{Write, BufRead};
use std::path::Path;
use chrono::{DateTime, Utc, TimeZone, Duration};

fn main() {
    let path = Path::new(DIR_DATA_COUNTRY_FLAGS);
    if !path.exists() {
        fs::create_dir_all(&path);
    }

    let path = Path::new(DIR_DATA_TERRITORY_FLAGS);
    if !path.exists() {
        fs::create_dir_all(&path);
    }

    let mut country_codes: Vec<String> = Vec::new();
    let html = attohttpc::get(URL_WIKI_ISO_3166_1).send().unwrap();
    let wiki_page = Html::parse_fragment(&html.text().unwrap());
    let last_modified = get_page_last_modified("ISO-3166-1", &wiki_page);
    if should_crawl(last_modified, FILE_LAST_MOD_COUNTRY) {
        country_codes = crawl_countries(wiki_page);
        fs::write(FILE_LAST_MOD_COUNTRY, last_modified.to_rfc3339());
    } else {
        println!("ISO-3166-1 is up to date, skipping...");
        country_codes = get_current_country_codes();
    }

    crawl_territories(country_codes);
}

fn should_crawl(last_modified: DateTime<Utc>, file_path: &str) -> bool {
    let path = Path::new(file_path);
    if !path.exists() {
        return true;
    }

    let current_last_modified_string = fs::read_to_string(path).unwrap();
    let current_last_modified = DateTime::parse_from_rfc3339(current_last_modified_string.as_str());
    if current_last_modified.is_err() {
        return true;
    }

    if last_modified <= current_last_modified.unwrap() {
        return false;
    }

    true
}

fn get_page_last_modified(page_name: &str, page: &Html) -> DateTime<Utc> {
    let selector_last_mod = Selector::parse("#footer-info-lastmod").unwrap();
    let element = page.select(&selector_last_mod).next().unwrap();
    let last_mod = element
        .text()
        .next()
        .unwrap()
        .replace("This page was last edited on", "");

    println!("{} Last Modified: {}", page_name, last_mod.trim());
    let parse_result = Utc.datetime_from_str(&last_mod.trim(), "%e %B %Y, at %k:%M");
    if parse_result.is_err() {
        panic!("Could not parse last modified on wiki page {}", page_name);
    }

    parse_result.unwrap()
}

fn get_current_country_codes() -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    let file = File::open(FILE_DATA_COUNTRY).unwrap();
    for line in io::BufReader::new(file).lines() {
        if let Ok(csv_line) = line {
            let mut parts = csv_line.split('\t');
            let code = parts.next().unwrap();
            if code.len() > 2 {
                continue;
            }

            codes.push(code.to_string());
        }
    }

    codes
}

fn crawl_countries(html: Html) -> Vec<String> {
    let mut countries_file = File::create(FILE_DATA_COUNTRY).unwrap();
    let selector_table = Selector::parse("table.wikitable tbody tr").unwrap();
    let selector_flag = Selector::parse("td:nth-child(1) img").unwrap();
    let selector_name = Selector::parse("td:nth-child(1) a").unwrap();
    let selector_alpha2 = Selector::parse("td:nth-child(2) a span").unwrap();
    let selector_alpha3 = Selector::parse("td:nth-child(3) span").unwrap();
    let selector_numeric = Selector::parse("td:nth-child(4) span").unwrap();
    let selector_iso_3166_2 = Selector::parse("td:nth-child(5) a").unwrap();

    println!("Processing countries...");
    countries_file.write("alpha2\talpha3\tnumeric\tname\tflag\n".as_bytes());
    let mut alpha2_codes: Vec<String> = Vec::new();
    for row in html.select(&selector_table) {
        if row.select(&selector_iso_3166_2).count() == 0 {
            continue;
        }

        let name = row.select(&selector_name).next().unwrap().inner_html();
        let alpha2 = row.select(&selector_alpha2).next().unwrap().inner_html();
        let alpha3 = row.select(&selector_alpha3).next().unwrap().inner_html();
        let numeric = row.select(&selector_numeric).next().unwrap().inner_html();

        println!("{} {}", alpha2, name);
        // TODO: Download emblems
        let flag_filename = format!("{}/{}.svg", DIR_DATA_COUNTRY_FLAGS, alpha2);
        download_flag(&flag_filename, row.select(&selector_flag).next().unwrap());

        let csv_line = format!("{}\t{}\t{}\t{}\t{}\n", alpha2, alpha3, numeric, name, flag_filename);
        countries_file.write(csv_line.as_bytes());
        alpha2_codes.push(alpha2.clone());
    }

    alpha2_codes
}

fn download_flag(svg_file: &String, flag: ElementRef) {
    let flag_url = flag.value().attr("src").unwrap();
    let parts  = flag_url.split('/').collect::<Vec<&str>>();
    let svg_url = format!("{}/{}/{}/{}", URL_WIKI_COMMONS, parts[6], parts[7], parts[8]);
    let svg = attohttpc::get(svg_url).send().unwrap();
    fs::write(svg_file, svg.text().unwrap());
}

fn crawl_territories(country_codes: Vec<String>) {
    println!("Processing territories...");
    let code = "CA";

    let response  = attohttpc::get(format!("{}:{}", URL_WIKI_ISO_3166_2, code)).send().unwrap();
    let html = Html::parse_fragment(&response.text().unwrap());
    let last_modified_file = format!("{}_{}", FILE_LAST_MOD_TERRITORY, code.to_lowercase());
    let last_modified = get_page_last_modified(format!("ISO-3166-2:{}", code).as_str(), &wiki_page);
    if !should_crawl(last_modified, last_modified_file.as_str()) {
        return;
    }

    let selector_table = Selector::parse("table.wikitable tbody tr").unwrap();
    let selector_flag = Selector::parse("td:nth-child(1) span img").unwrap();
    let selector_name = Selector::parse("td:nth-child(1) a").unwrap();
    let selector_alpha2 = Selector::parse("td:nth-child(2) a span").unwrap();
    let path = Path::new(FILE_DATA_TERRITORY);
    if path.exists() {
        cleanup_territory(&path)
    }

    let mut territories_file = File::create(path).unwrap();
    territories_file.write("alpha2\tname\tflag\n".as_bytes());
    for row in html.select(&selector_table) {
        let name = row.select(&selector_name).next().unwrap().inner_html();
        let code = row.select(&selector_alpha2).next().unwrap().inner_html();

        println!("{} {}", code, name);
        let mut flag_filename = String::from("");
        let img = row.select(&selector_flag).next();
        if let Some(flag) = img {
            flag_filename = format!("{}/{}.svg", DIR_DATA_TERRITORY_FLAGS, code);
            download_flag(&flag_filename, flag);
        }

        let csv_line = format!("{}\t{}\t{}\n", code, name, flag_filename);
        territories_file.write(csv_line.as_bytes());
    }

    fs::write(last_modified_file, last_modified.to_rfc3339());
}

fn cleanup_territory(file: &Path) {

}