#![allow(dead_code)]

use std::{
    io::{self},
    str::Lines,
};

type HttpResult<T> = Result<T, HttpError>;

// TODO
#[derive(Debug)]
pub struct HttpError;

#[derive(Debug, Clone, Copy)]
pub enum HttpVersion {
    V1_0,
    V1_1,
    V2,
}

pub enum HttpStatusCode {
    // 200
    Ok,
    // 404
    NotFound,
    // 500
    InternalServerError,
    Other(u16, String),
}

impl From<HttpVersion> for String {
    fn from(version: HttpVersion) -> Self {
        let version = match version {
            HttpVersion::V1_0 => "HTTP/1.0",
            HttpVersion::V1_1 => "HTTP/1.1",
            HttpVersion::V2 => todo!(),
        };
        String::from(version)
    }
}

impl HttpStatusCode {
    fn reason_phrase(&self) -> &str {
        match self {
            HttpStatusCode::Ok => "200 OK",
            HttpStatusCode::NotFound => "Not Found",
            HttpStatusCode::InternalServerError => "Internal Server Error",
            HttpStatusCode::Other(_, reason_phrase) => reason_phrase,
        }
    }
}
impl From<&HttpStatusCode> for String {
    fn from(code: &HttpStatusCode) -> Self {
        let code = match code {
            HttpStatusCode::Ok => 200,
            HttpStatusCode::NotFound => 404,
            HttpStatusCode::InternalServerError => 500,
            HttpStatusCode::Other(n, _) => *n,
        };
        code.to_string()
    }
}

impl From<HttpStatusCode> for u16 {
    fn from(code: HttpStatusCode) -> Self {
        match code {
            HttpStatusCode::Ok => 200,
            HttpStatusCode::NotFound => 404,
            HttpStatusCode::InternalServerError => 500,
            HttpStatusCode::Other(n, _) => n,
        }
    }
}

pub enum Request {
    Index,
    Data,
    Favicon,
    Css(String),
    Image(String),
}

/// Intended only for GET method
pub struct HttpRequest {
    request: Request,
    version: HttpVersion,
    headers: Option<String>,
    body: Option<String>,
}

pub struct HttpResponse {
    version: HttpVersion,
    code: HttpStatusCode,
    headers: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub fn read_from<T: io::Read>(mut stream: T) -> HttpResult<HttpRequest> {
        let mut buff = [0u8; 1024];
        let _ = stream.read(&mut buff).unwrap();
        let tmp = String::from_utf8_lossy(&buff).replace("\r", "");
        let mut lines = tmp.lines();
        let mut request_line = lines.next().unwrap().split_whitespace();
        let method = request_line.next().unwrap();
        if method != "GET" {
            panic!("Error: metodo no es GET");
        }
        let request_uri = request_line.next().unwrap();
        let request = HttpRequest::parse_request_uri(request_uri).unwrap();
        let http_version = request_line.next().unwrap();
        let version = HttpRequest::parse_http_version(http_version).unwrap();
        let headers = HttpRequest::parse_headers(&mut lines).unwrap();
        let body = HttpRequest::parse_body(&mut lines).unwrap();

        Ok(HttpRequest {
            request,
            version,
            headers,
            body,
        })
    }

    fn parse_headers(lines: &mut Lines) -> HttpResult<Option<String>> {
        let mut headers = String::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            headers.push_str(line);
        }
        if headers.is_empty() {
            Ok(None)
        } else {
            Ok(Some(headers))
        }
    }

    fn parse_body(lines: &mut Lines) -> HttpResult<Option<String>> {
        let mut body = String::new();
        for line in lines {
            body.push_str(line);
        }
        if body.is_empty() {
            Ok(None)
        } else {
            Ok(Some(body))
        }
    }

    fn parse_http_version(http_version: &str) -> HttpResult<HttpVersion> {
        match http_version {
            "HTTP/1.1" => Ok(HttpVersion::V1_1),
            _ => panic!("Http version not supported: {}", http_version),
        }
    }

    fn parse_request_uri(request_uri: &str) -> HttpResult<Request> {
        if !request_uri.contains('/') {
            panic!("URI invalida: {}", request_uri);
        }
        if request_uri == "/" {
            Ok(Request::Index)
        } else if request_uri == "/data" {
            Ok(Request::Data)
        } else if request_uri == "/favicon.ico" {
            Ok(Request::Favicon)
        } else if let Some(stripped) = request_uri.strip_prefix("/resources/css/") {
            let filename = stripped;
            Ok(Request::Css(filename.to_owned()))
        } else if let Some(stripped) = request_uri.strip_prefix("/resources/image/") {
            let filename = stripped;
            Ok(Request::Image(filename.to_owned()))
        } else {
            panic!("URI invalida: {}", request_uri);
        }
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn version(&self) -> &HttpVersion {
        &self.version
    }

    pub fn headers(&self) -> Option<&str> {
        self.headers.as_deref()
    }
}

impl HttpResponse {
    pub fn new<U, V>(
        code: HttpStatusCode,
        version: HttpVersion,
        headers: Option<U>,
        body: Option<V>,
    ) -> HttpResponse
    where
        U: Into<Vec<u8>>,
        V: Into<Vec<u8>>,
    {
        let headers = headers.map(|x| x.into());
        let body = body.map(|x| x.into());
        HttpResponse {
            version,
            code,
            headers,
            body,
        }
    }

    fn body_len(&self) -> usize {
        if let Some(body) = self.body.as_deref() {
            body.len()
        } else {
            0
        }
    }
    pub fn send_to<T: io::Write>(&self, stream: &mut T) -> HttpResult<()> {
        let mut buff = vec![];
        let response = format!(
            "{} {} {}\r\nContent-Length: {}\r\n",
            String::from(self.version),
            self.code.reason_phrase(),
            String::from(&self.code),
            self.body_len()
        );

        stream.write_all(response.as_bytes()).unwrap();
        buff.append(&mut response.as_bytes().to_vec());

        let headers = self.headers.as_deref().unwrap_or_else(|| "".as_bytes());
        let body = self.body.as_deref().unwrap_or(&[]);
        stream.write_all(headers).unwrap();
        stream.write_all("\r\n".as_bytes()).unwrap();
        buff.append(&mut headers.to_vec());
        buff.append(&mut "\r\n".as_bytes().to_vec());

        stream.write_all(body).unwrap();
        buff.append(&mut body.to_vec());

        stream.flush().unwrap();
        println!("RESPONSE:\n{}", String::from_utf8_lossy(&buff[..]));
        Ok(())
    }
}
