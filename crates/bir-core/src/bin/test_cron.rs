use std::str::FromStr;

fn main() {
    let expr = "* * * * *";
    match cron::Schedule::from_str(expr) {
        Ok(_) => println!("5 stars works"),
        Err(e) => println!("5 stars fails: {}", e),
    }

    let expr6 = "0 * * * * *";
    match cron::Schedule::from_str(expr6) {
        Ok(_) => println!("6 parts works"),
        Err(e) => println!("6 parts fails: {}", e),
    }
}
