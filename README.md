# gitlab-push-and-mr

Push and create an MR automatically using gitlab API and GIT.

You need a Gitlab account and a project there, plus an API key.

## Version

This project is a fork of https://github.com/zupzup/gitlab-push-and-mr customized to my needs.

Here is the article related to the above project: https://www.zupzup.org/async-awaitify-rust-cli

Changes compared to original code:
* All parameters are specified in a toml configuration file (describe below).
* Push phase authentication can be done using either user and password or ssh keys.
* Merge request can be assign to someone as an option.
* Update dependencies to the latest available stable revision

## Run

All parameters must be configured into $HOME/.glpm/config.toml file:
```toml
user = "user_name"
password = "user_password"
ssh_key_file = "/home/user_name/.ssh/id_rsa"
ssh_passphrase = "user_passphrase"
apikey="gitlab_api_key"
mr_labels = ["DevOps"]
host = "http://gitlab.example.com"
```

If password key is defined, user and password will be used to perform the authentication. Otherwise, it will use ssh_key_file and ssh_passphrase configuration keys.

## Usage examples
Execute:
```bash
// run tool
cargo run -- -d "Some Description" -t "Some Title"
```

```bash
gitlab-push-and-mr -t "Some title" -a username -b main
```
