use crate::data::{Config, MRPayload, MRRequest, MRResponse, ProjectResponse, User};
use hyper::{Body, Client, Method, Request};
use hyper_tls::HttpsConnector;
use futures::{future};
use hyper::client::HttpConnector;
use std::error::Error;
use std::fmt;
use hyper::body::Bytes;

type Result<T> = std::result::Result<T, HttpError>;

#[derive(Debug)]
pub enum HttpError {
    UnsuccessFulError(hyper::StatusCode),
    ConfigError(),
    HyperError(hyper::Error),
    HyperHttpError(hyper::http::Error),
    JsonError(serde_json::Error),
}

impl Error for HttpError {
    fn description(&self) -> &str {
        match *self {
            HttpError::UnsuccessFulError(..) => "unsuccessful request",
            HttpError::ConfigError(..) => "invalid config provided - no group",
            HttpError::HyperError(..) => "hyper error",
            HttpError::HyperHttpError(..) => "hyper http error",
            HttpError::JsonError(..) => "serde json error",
        }
    }
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            HttpError::UnsuccessFulError(..) => None,
            HttpError::ConfigError(..) => None,
            HttpError::HyperError(ref e) => Some(e),
            HttpError::HyperHttpError(ref e) => Some(e),
            HttpError::JsonError(ref e) => Some(e),
        }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}: ", self.description())?;
            match *self {
                        HttpError::UnsuccessFulError(ref v) => write!(f, "unsuccessful request: {}", v),
                        HttpError::ConfigError(..) => write!(f, "invalid config found - no group"),
                        HttpError::HyperError(ref e) => write!(f, "{}", e),
                        HttpError::HyperHttpError(ref e) => write!(f, "{}", e),
                        HttpError::JsonError(ref e) => write!(f, "{}", e),
                    }
        }
}

impl From<hyper::Error> for HttpError {
    fn from(e: hyper::Error) -> Self {
        HttpError::HyperError(e)
    }
}

impl From<hyper::http::Error> for HttpError {
    fn from(e: hyper::http::Error) -> Self {
        HttpError::HyperHttpError(e)
    }
}

impl From<serde_json::Error> for HttpError {
    fn from(e: serde_json::Error) -> Self {
        HttpError::JsonError(e)
    }
}

pub async fn fetch_projects(
    config: &Config,
    access_token: &str,
    domain: &str,
) -> Result<Vec<ProjectResponse>> {
    let projects_raw = fetch(config, access_token, domain, 20).await?;
    let mut result: Vec<ProjectResponse> = Vec::new();
    for p in projects_raw {
        let mut data: Vec<ProjectResponse> = serde_json::from_slice(&p)?;
        result.append(&mut data);
    }
    Ok(result)
}

async fn fetch(
    config: &Config,
    access_token: &str,
    domain: &str,
    per_page: i32,
) -> Result<Vec<Bytes>> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let group = config.group.as_ref();
    let user = config.user.as_ref();
    let host = config.host.as_ref();
    let uri = match group {
        Some(v) => format!(
            "{}/api/v4/groups/{}/{}?per_page={}",
            host.unwrap_or(&"https://gitlab.com".to_string()), v, domain, per_page
        ),
        None => match user {
            Some(u) => format!(
                "{}/api/v4/users/{}/{}?per_page={}",
                host.unwrap_or(&"https://gitlab.com".to_string()), u, domain, per_page
            ),
            None => "invalid url".to_string(),
        },
    };
    let req = Request::builder()
        .uri(uri)
        .header("PRIVATE-TOKEN", access_token.to_owned())
        .body(Body::empty())?;
    let res = client.request(req).await?;
    if !res.status().is_success() {
        return Err(HttpError::UnsuccessFulError(res.status()));
    }
    let pages: &str = match res.headers().get("x-total-pages") {
        Some(v) => match v.to_str() {
            Ok(v) => v,
            _ => "0",
        },
        None => "0",
    };
    let p = match pages.parse::<i32>() {
        Ok(v) => v,
        Err(_) => 0,
    };
    let mut result: Vec<Bytes> = Vec::new();
    let body = hyper::body::to_bytes(res.into_body()).await?;
    result.push(body);
    let mut futrs = Vec::new();
    for page in 2..=p {
        futrs.push(fetch_paged(&config, &access_token, &domain, &client, page));
    }
    let paged_results = future::join_all(futrs).await;
    for r in paged_results {
        let str = match r {
            Ok(v) => v,
            Err(_) => return Err(HttpError::UnsuccessFulError(hyper::StatusCode::INTERNAL_SERVER_ERROR)),
        };
        result.push(str);
    }
    Ok(result)
}

async fn fetch_paged(
    config: &Config,
    access_token: &str,
    domain: &str,
    client: &hyper::Client<HttpsConnector<HttpConnector>>,
    page: i32,
) -> Result<Bytes> {
    let host = config.host.as_ref();
    let group = match config.group.as_ref() {
        Some(v) => v,
        None => return Err(HttpError::ConfigError())
    };
    let req = Request::builder()
        .uri(format!(
            "{}/api/v4/groups/{}/{}?per_page=20&page={}",
            host.unwrap_or(&"https://gitlab.com".to_string()), group, domain, page
        ))
        .header("PRIVATE-TOKEN", access_token)
        .body(Body::empty())?;
    let res = client.request(req).await?;
    if !res.status().is_success() {
        return Err(HttpError::UnsuccessFulError(res.status()));
    }
    let body = hyper::body::to_bytes(res.into_body()).await?;
    Ok(body)
}

pub async fn fetch_users(
    config: &Config,
    access_token: &str,
    assignee: &str,
) -> Result<Vec<User>> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let host = config.host.as_ref();
    let req = Request::builder()
        .uri(format!(
            "{}/api/v4/users?search={}",
            host.unwrap_or(&"https://gitlab.com".to_string()), assignee
        ))
        .header("PRIVATE-TOKEN", access_token)
        .body(Body::empty())?;
    // println!("{:?}", &req);
    let res = client.request(req).await?;
    if !res.status().is_success() {
        return Err(HttpError::UnsuccessFulError(res.status()));
    }
    let body = hyper::body::to_bytes(res.into_body()).await?;
    let data: Vec<User> = serde_json::from_slice(&body)?;
    Ok(data)
}

pub async fn create_mr(payload: &MRRequest<'_>, config: &Config) -> Result<String> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let host = config.host.as_ref();
    let uri = format!(
        "{}/api/v4/projects/{}/merge_requests",
        host.unwrap_or(&"https://gitlab.com".to_string()), payload.project.id
    );
    let labels = config
        .mr_labels
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .fold(String::new(), |acc, l| format!("{}, {}", acc, l));

    let mr_payload = MRPayload {
        id: &format!("{}", payload.project.id),
        title: &payload.title,
        description: &payload.description,
        target_branch: &payload.target_branch,
        source_branch: &payload.source_branch,
        labels: &labels,
        squash: true,
        remove_source_branch: true,
        assignee_id : payload.assignee_id,
    };
    let json = serde_json::to_string(&mr_payload)?;
    let req = Request::builder()
        .uri(uri)
        .header("PRIVATE-TOKEN", payload.access_token.to_owned())
        .header("Content-Type", "application/json")
        .method(Method::POST)
        .body(Body::from(json))?;
    let res = client.request(req).await?;
    if !res.status().is_success() {
        return Err(HttpError::UnsuccessFulError(res.status()));
    }
    let body = hyper::body::to_bytes(res.into_body()).await?;
    let data: MRResponse = serde_json::from_slice(&body)?;
    Ok(data.web_url)
}
