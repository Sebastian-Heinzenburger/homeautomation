extern crate chrono;

use chrono::NaiveDateTime;
use rand::distributions::Alphanumeric;
use rand::Rng;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::HeaderMap;
use std::convert::TryInto;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

//// CONFIGURATION ////
static CMD_PASSWORD: &str = "secret"; //the AES key the commands are end-to-end encrypted with
static EMAIL_ADDR: &str = "user@example.com";
static EMAIL_BIN: &str = "neomutt"; //or mailx e.g.
static BASE_URL: &str = "https://home.example.com/";

static HTTP_USER: &str = "automatipi"; //set in .htacces for example
static HTTP_PASS: &str = "secret";

static REFRESH_DELAY: Duration = Duration::from_secs(10);
///////////////////////

struct Auth {
    user: String,
    pass: String,
}

struct HomeCmd {
    time: NaiveDateTime,
    text: String,
}

struct HomeCmdAwaitingCheck {
    check_pair: CheckPair,
    command: HomeCmd,
}

struct CheckPair {
    identifier: String,
    check_code: String,
}

trait HTTPAuthable {
    fn http_auth(&self) -> HeaderMap;
}

impl HTTPAuthable for Auth {
    fn http_auth(&self) -> HeaderMap {
        let mut auth_header = HeaderMap::new();
        auth_header.append(
            "Authorization",
            ("Basic ".to_owned() + &*base64::encode((&*self.user).to_owned() + ":" + &*self.pass))
                .parse()
                .unwrap(),
        );
        auth_header
    }
}

impl CheckPair {
    fn new() -> CheckPair {
        CheckPair {
            check_code: random_code(),
            identifier: random_code(),
        }
    }
}

impl Clone for HomeCmd {
    fn clone(&self) -> HomeCmd {
        HomeCmd {
            time: self.time,
            text: "".to_owned() + &self.text,
        }
    }
}

///send mail with subject "homeautomation" to EMAIL_ADDR
fn send_mail(text: &str) {
    Command::new(EMAIL_BIN.to_owned())
        .args(["-s", "homeautomation", EMAIL_ADDR])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .stdin
        .as_ref()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

///decrypt cmd via openssl with 265-bit aes-cbc encryption
fn decrypt_cmd(cmd: &str) -> Option<HomeCmd> {
    let proc = Command::new("openssl")
        .args([
            "enc",
            "-d",
            "-aes-256-cbc",
            "-in",
            "-",
            "-out",
            "-",
            "-pass",
            &("pass:".to_owned() + &*CMD_PASSWORD.to_owned()),
            "-base64",
            "-md",
            "md5",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    proc.stdin
        .as_ref()
        .unwrap()
        .write_all((cmd.to_owned() + &*"\r\n".to_owned()).as_bytes())
        .unwrap();

    let openssl_output = proc.wait_with_output().unwrap();
    if openssl_output.status.success() {
        let mut cleartext = String::from_utf8(openssl_output.stdout).unwrap();

        //check if the command has been tampered with
        if !cleartext.starts_with("CMD") {
            return None;
        }

        //construct command from cleartext
        cleartext = cleartext.trim_start_matches("CMD").to_owned();
        let time_and_text = cleartext.split_once(":").unwrap();
        return Some(HomeCmd {
            time: NaiveDateTime::from_timestamp(
                time_and_text.0[..time_and_text.0.len() - 3]
                    .parse::<i64>()
                    .unwrap(),
                0,
            ),
            text: time_and_text.1.to_string(),
        });
    }
    return None;
}

fn fetch_commands(client: &Client, auth: &Auth) -> Vec<HomeCmd> {
    //because of rusts ownership we need to create a copy
    let mut auth_header = HeaderMap::default();
    auth_header.clone_from(&auth.http_auth());

    let text = client
        .get(BASE_URL.to_owned() + "out.php")
        .headers(auth.http_auth())
        .send()
        .unwrap()
        .text()
        .unwrap();

    let mut commands: Vec<HomeCmd> = Vec::new();
    for line in text.split("\n") {
        let cmd = decrypt_cmd(line);
        match cmd {
            Some(c) => commands.push(c),
            None => {}
        }
    }

    commands
}

///returns the commands in cmds that are sceduled after last_time
fn current_commands(cmds: Vec<HomeCmd>, last_time: NaiveDateTime) -> Vec<HomeCmd> {
    let mut fut_cmds = Vec::<HomeCmd>::new();
    for c in cmds {
        if c.time.timestamp() >= last_time.timestamp() {
            fut_cmds.push(c);
        }
    }

    fut_cmds
}

///fetches the pairs of identifier and check code from the server
fn fetch_check_pairs(client: &Client, auth: &Auth) -> Vec<CheckPair> {
    let mut auth_header = HeaderMap::default();
    auth_header.clone_from(&auth.http_auth());
    let text = client
        .get(BASE_URL.to_owned() + "confirm_out.php")
        .headers(auth.http_auth())
        .send()
        .unwrap()
        .text()
        .unwrap();

    let mut pairs = Vec::<CheckPair>::new();
    for line in text.split("\n") {
        let pair = line.split_once(":").unwrap();
        pairs.push(CheckPair {
            identifier: pair.0.to_owned(),
            check_code: pair.1.to_owned(),
        });
    }
    pairs
}

fn current_sys_time() -> NaiveDateTime {
    NaiveDateTime::from_timestamp(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .try_into()
            .unwrap(),
        0,
    )
}

fn random_code() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect::<String>()
}

fn main() {
    let auth = Auth {
        user: HTTP_USER.to_owned(),
        pass: HTTP_PASS.to_owned(),
    };
    let client = ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::default())
        .build()
        .unwrap();

    let mut last_time = current_sys_time(); //the last time we fetched commands from the server
    let mut cmds_awaiting_check = Vec::<HomeCmdAwaitingCheck>::new();
    let mut cmds_awaiting_exec = Vec::<HomeCmd>::new();

    loop {
        let check_pairs: Vec<CheckPair> = fetch_check_pairs(&client, &auth);
        let mut indexes_to_be_removed_awaiting = Vec::<usize>::new();
        let mut indexes_to_be_removed_executing = Vec::<usize>::new();


        //see if any outstanding commands have been validated
        //and then add them to the queue to be executed
        for (i, c) in cmds_awaiting_check.iter().enumerate() {
            for p in &check_pairs {
                if p.check_code == c.check_pair.check_code
                    && p.identifier == c.check_pair.identifier
                {
                    eprintln!("[LOG] validated command: {}", c.command.text);
                    indexes_to_be_removed_awaiting.push(i);
                    cmds_awaiting_exec.push(c.command.clone());
                }
            }
        }

        //remove the validated commands from the queue
        for i in indexes_to_be_removed_awaiting {
            cmds_awaiting_check.remove(i);
        }


        //execute outstanding commands
        for (i, c) in cmds_awaiting_exec.iter().enumerate() {
            if c.time.timestamp() < current_sys_time().timestamp() {
                indexes_to_be_removed_executing.push(i);
                println!("{}", c.text);
            }
        }

        //remove the validated commands from the queue
        for i in indexes_to_be_removed_executing {
            cmds_awaiting_exec.remove(i);
        }


        //fetch new commands from the server
        let cmds = fetch_commands(&client, &auth);
        let relevant_cmds = current_commands(cmds, last_time);

        for c in relevant_cmds {
            eprintln!(
                "[LOG] received new command {:?}: {}",
                c.time.timestamp(),
                c.text
            );
            let check_pair = CheckPair::new();
            send_mail(&format!("would you like to execute: \"{}\"? then click on this link: https://home.heinzenburger.de/confirm.php?i={}&c={}", c.text, check_pair.identifier, check_pair.check_code));
            eprintln!(
                "[LOG] generated identifier {} and check_code: {}",
                check_pair.identifier, check_pair.check_code
            );

            cmds_awaiting_check.push(HomeCmdAwaitingCheck {
                check_pair: check_pair,
                command: c,
            });
        }

        last_time = current_sys_time();
        thread::sleep(REFRESH_DELAY);
    }
}
