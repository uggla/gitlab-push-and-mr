extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate serde_derive;
extern crate clap;
extern crate serde;
extern crate serde_json;
extern crate toml;

use clap::{App, Arg};
use data::{Config, MRRequest, ProjectResponse};
use error::AppError;
use git2::{PushOptions, RemoteCallbacks, Repository};
use std::env;
use std::fs::{self};
use tokio::runtime::Runtime;

mod data;
mod error;
mod http;

const CONFIG_FILE: &str = ".glpm/config.toml";

type Result<T> = std::result::Result<T, AppError>;

fn git_credentials_ssh_callback(
    _user: &str,
    user_from_url: Option<&str>,
    cred: git2::CredentialType,
) -> std::result::Result<git2::Cred, git2::Error> {
    let user = user_from_url.unwrap_or("git");

    if cred.contains(git2::CredentialType::USERNAME) {
        return git2::Cred::username(user);
    }
    let config = get_config().expect("Could not read config");
    let key_file = &config.ssh_key_file.unwrap();
    let passphrase = &config.ssh_passphrase.unwrap();
    git2::Cred::ssh_key(
        user,
        None,
        std::path::Path::new(&key_file),
        Some(passphrase),
    )
}

fn git_credentials_pwd_callback(
    _user: &str,
    _user_from_url: Option<&str>,
    _cred: git2::CredentialType,
) -> std::result::Result<git2::Cred, git2::Error> {
    let config = get_config().expect("Could not read config");
    git2::Cred::userpass_plaintext(&config.user.unwrap(), &config.password.unwrap())
}

fn get_config() -> Result<Config> {
    let config_file: &str =
        &(env::var("HOME").expect("Cannot find HOME environment variable") + "/" + CONFIG_FILE);

    let data = fs::read_to_string(config_file)?;
    let config: Config = toml::from_str(&data)?;
    Ok(config)
}

fn get_current_branch(repo: &Repository) -> Result<String> {
    let branches = repo.branches(None).expect("can't list branches");
    branches.fold(
        Err(AppError::Git(String::from("current branch not found"))),
        |acc, branch| {
            let b = branch.map_err(|_| AppError::Git(String::from("current branch not found")))?;
            if b.0.is_head() {
                let name =
                    b.0.name()
                        .map_err(|_| AppError::Git(String::from("current branch not found")))?;
                return match name {
                    Some(n) => Ok(n.to_string()),
                    None => return acc,
                };
            }
            acc
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn create_mr(
    config: &Config,
    actual_remote: &str,
    access_token: &str,
    title: &str,
    description: &str,
    target_branch: &str,
    current_branch: &str,
    assignee: &str,
) {
    let mut rt = Runtime::new().expect("Tokio runtime can be initialized");
    rt.block_on(async move {
        let mut assignee_id: Option<u64> = None;
        let parsed_id = assignee.parse::<u64>();
        if let Ok(id) = parsed_id {
            assignee_id = Some(id)
        };

        // Check if we pass an assignee
        if !assignee.is_empty() && assignee_id == None {
            let users = match http::fetch_users(config, access_token, assignee).await {
                Ok(u) => u,
                Err(e) => return println!("Could not fetch users, reason: {}", e),
            };
            match users.len() {
                x if x > 1 => {
                    println!("Available users:");
                    println!("----------------");
                    println!();
                    for user in users {
                        println!("id: {}", user.id);
                        println!("User: {}", user.name);
                        println!("Username: {}", user.username);
                        println!("--------------------");
                    }
                    return println!(
                        "Assignee is not unique, please refine your query or use an id"
                    );
                }
                x if x == 1 => {
                    assignee_id = Some(users[0].id);
                }
                x if x < 1 => {
                    return println!("Assignee not found, please check assignee name or id");
                }
                _ => {} // Just to make the compiler happy
            }
        }

        let projects = match http::fetch_projects(config, access_token, "projects").await {
            Ok(v) => v,
            Err(e) => return println!("Could not fetch projects, reason: {}", e),
        };
        let mut actual_project: Option<&ProjectResponse> = None;
        for p in &projects {
            if p.ssh_url_to_repo == actual_remote {
                actual_project = Some(p);
                break;
            }
            if p.http_url_to_repo == actual_remote {
                actual_project = Some(p);
                break;
            }
        }
        let project = actual_project.expect("Couldn't find this project on gitlab");
        let mr_req = MRRequest {
            access_token,
            project,
            title,
            description,
            source_branch: current_branch,
            target_branch,
            assignee_id,
        };
        match http::create_mr(&mr_req, config).await {
            Ok(v) => println!("Pushed and Created MR Successfully - URL: {}", v),
            Err(e) => println!("Could not create MR, Error: {}", e),
        };
    });
}

fn main() -> Result<()> {
    let matches = App::new("Gitlab Push-and-MR")
        .version("1.3.0")
        .arg(
            Arg::with_name("description")
                .short("d")
                .long("description")
                .value_name("DESCRIPTION")
                .help("The Merge-Request description")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("title")
                .short("t")
                .required(true)
                .long("title")
                .value_name("TITLE")
                .help("The Merge-Request title")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("target_branch")
                .short("b")
                .long("target-branch")
                .value_name("TARGETBRANCH")
                .help("The Merge-Request target branch")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("assignee")
                .short("a")
                .long("assignee")
                .value_name("ASSIGNEE")
                .help("The Merge-Request assignee user")
                .takes_value(true),
        )
        .get_matches();
    let title = matches
        .value_of("title")
        .expect("title needs to be provided");
    let description = matches.value_of("description").unwrap_or("");
    let target_branch = matches.value_of("target_branch").unwrap_or("master");

    let config = get_config().expect("Could not read config file");
    let access_token = config.clone().apikey.expect("Could not get access token");

    if config.group.is_none() && config.user.is_none() {
        panic!("Group or User for Gitlab need to be configured")
    }

    let assignee = matches.value_of("assignee").unwrap_or("");
    let repo = Repository::open("./").expect("Current folder is not a git repository");
    let current_branch = get_current_branch(&repo).expect("Could not get current branch");
    let mut remote = repo
        .find_remote("origin")
        .expect("Origin remote could not be found");

    let mut push_opts = PushOptions::new();
    let mut callbacks = RemoteCallbacks::new();
    let actual_remote = String::from(remote.url().expect("Could not get remote URL"));
    let branch_clone = current_branch.clone();
    if config.password.is_none() {
        callbacks.credentials(git_credentials_ssh_callback);
    } else {
        callbacks.credentials(git_credentials_pwd_callback);
    }
    callbacks.push_update_reference(move |refname, _| {
        println!("Successfully Pushed: {:?}", refname);
        create_mr(
            &config,
            &actual_remote,
            &access_token,
            title,
            description,
            target_branch,
            &branch_clone,
            assignee,
        );
        Ok(())
    });
    push_opts.remote_callbacks(callbacks);
    remote
        .push(
            &[&format!("refs/heads/{}", current_branch)],
            Some(&mut push_opts),
        )
        .expect("Could not push to origin");
    Ok(())
}
