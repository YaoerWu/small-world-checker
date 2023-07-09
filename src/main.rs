use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Display,
    fs::{File, OpenOptions},
    io::Read,
    io::Write,
    path::Path,
};

fn pause() {
    println!("Press ENTER to continue...");
    let buffer = &mut [0u8];
    std::io::stdin().read_exact(buffer).unwrap();
}

fn main() {
    let arg = env::args().nth(1).unwrap_or_else(|| {
        println!("你需要把你的卡组文件拖到程序上来使用");
        pause();
        panic!();
    });
    let card_set_path = Path::new(&arg).to_owned();
    let download_path = card_set_path.parent().unwrap().join(Path::new("cards.zip"));
    if let Ok(data) = get_data(&download_path) {
        let card_set = init_card_set(init_database(data), &card_set_path).unwrap_or_else(|_| {
            println!("无法读取卡组，请检查卡组格式");
            pause();
            panic!();
        });
        let out = check_connect(card_set);
        println!("{}", out);
        let mut file = File::create("out.txt").unwrap();
        writeln!(file, "{}", out).unwrap();
        pause();
    } else {
        println!("无法下载或打开牌库，请尝试手动下载\n下载地址为：\nhttps://ygocdb.com/api/v0/cards.zip\n下载完成后置于下列路径\n{}",download_path.display());
        pause();
        panic!();
    }
}

fn init_card_set(database: HashMap<i32, Card>, path: &Path) -> Result<HashSet<Card>> {
    let file = OpenOptions::new().read(true).open(path);
    let mut list = String::new();
    let _ = Read::read_to_string(&mut file.unwrap(), &mut list);
    let mut card_set = HashSet::new();
    for i in list.lines().skip(2) {
        if i.starts_with('#') {
            break;
        }
        if let Some(card) = database.get(&i.parse()?) {
            card_set.insert(card.clone());
        }
    }
    if card_set.is_empty() {
        anyhow::bail!("");
    }
    Ok(card_set)
}
fn init_database(data: String) -> HashMap<i32, Card> {
    // let order = ["nwbbs_n", "cn_name", "md_name"];
    let mut database = HashMap::new();
    json::parse(&data).unwrap().entries().for_each(|(_, x)| {
        let id = x["id"].as_i32().unwrap();
        if id != 0 {
            let stats: [i32; 5] = x["data"]
                .entries()
                .skip(3)
                .map(|(_, x)| x.as_i32().unwrap())
                .collect::<Vec<i32>>()
                .try_into()
                .unwrap();
            if stats.iter().copied().sum::<i32>() != 0 {
                database.insert(
                    id,
                    Card {
                        id,
                        name: { x["nwbbs_n"].as_str().unwrap_or("").to_string() },
                        five_stats: stats,
                    },
                );
            }
        }
    });
    database
}
fn get_data(path: &Path) -> Result<String> {
    if path.exists() {
        let str = unzip(File::open(path)?)?;
        if check_update(&str) {
            Ok(str)
        } else if let Ok(file) = download_data() {
            Ok(unzip(file)?)
        } else {
            Ok(str)
        }
    } else {
        Ok(unzip(download_data()?)?)
    }
}
fn check_update(str: &str) -> bool {
    let url = "https://ygocdb.com/api/v0/cards.zip.md5";
    let digest = md5::compute(str.as_bytes());
    if let Ok(resp) = reqwest::blocking::get(url) {
        format!("{:x}", digest) == resp.json::<String>().unwrap_or_default()
    } else {
        false
    }
}
fn download_data() -> Result<File> {
    let path = "./cards.zip";
    let url = "https://ygocdb.com/api/v0/cards.zip";
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;
    let _ = reqwest::blocking::get(url)?.copy_to(&mut file);
    Ok(file)
}
fn unzip(file: File) -> Result<String> {
    let mut zip = zip::ZipArchive::new(file)?;
    let mut file = zip.by_name("cards.json")?;
    let mut str = String::new();
    let _ = file.read_to_string(&mut str);
    Ok(str)
}

fn check_connect(card_set: HashSet<Card>) -> String {
    let mut table: HashMap<&Card, HashMap<&Card, Vec<&Card>>> = HashMap::new();
    for i in card_set.iter() {
        table.insert(i, HashMap::new());
    }
    use std::fmt::Write;
    let mut str = String::new();
    for (first_card, mut next) in table {
        writeln!(str, "{first_card}").unwrap();
        card_set.iter().for_each(|x| {
            if x.is_connected(first_card) {
                next.insert(x, Vec::new());
            }
        });

        let mut iter = next.iter_mut().peekable();
        while let Some((secend_card, next)) = iter.next() {
            let (second_connect_char, last_connect_char) = if iter.peek().is_some() {
                ('├', '│')
            } else {
                ('└', ' ')
            };
            writeln!(str, "    {second_connect_char}──{secend_card}").unwrap();
            card_set.iter().for_each(|x| {
                if x.is_connected(secend_card) && x != first_card {
                    next.push(x);
                }
            });

            let mut iter = next.iter_mut().peekable();
            while let Some(i) = iter.next() {
                let connect_char = if iter.peek().is_some() { '├' } else { '└' };
                writeln!(str, "    {last_connect_char}    {connect_char}──{i}").unwrap();
            }
        }
    }
    str
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Eq, Hash, Clone)]
struct Card {
    id: i32,
    name: String,
    five_stats: [i32; 5],
}
impl Card {
    fn is_connected(&self, other: &Self) -> bool {
        let mut same_field = 0;
        for i in self.five_stats.iter().zip(other.five_stats.iter()) {
            if i.0 == i.1 {
                same_field += 1;
            }
        }
        same_field == 1
    }
}
impl Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "{}", self.id)
        } else {
            write!(f, "{}", self.name)
        }
    }
}
