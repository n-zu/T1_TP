#![allow(dead_code)]

use std::{
    convert::TryFrom,
    error::Error,
    io::{self},
    str::Lines,
};

type HttpResult<T> = Result<T, HttpError>;
type HttpError = Box<dyn Error>;

const GET_METHOD: &str = "GET";

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

impl TryFrom<&str> for HttpVersion {
    type Error = HttpError;

    fn try_from(version: &str) -> HttpResult<Self> {
        let version = match version {
            "HTTP/1.0" => HttpVersion::V1_0,
            "HTTP/1.1" => HttpVersion::V1_1,
            "HTTP/2" => HttpVersion::V2,
            _ => return Err(HttpError::from("Version no soportada")),
        };
        Ok(version)
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

impl TryFrom<&str> for Request {
    type Error = HttpError;

    fn try_from(request_uri: &str) -> Result<Self, Self::Error> {
        if !request_uri.contains('/') {
            Err(format!("URI invalida: {}", request_uri).into())
        } else if request_uri == "/" {
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
            Err(format!("URI invalida: {}", request_uri).into())
        }
    }
}

/// Intended only for GET method
pub struct HttpRequest {
    request: Request,
    version: HttpVersion,
    headers: Vec<String>,
    body: Vec<String>,
}

pub struct HttpResponse {
    version: HttpVersion,
    code: HttpStatusCode,
    headers: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
}

impl HttpRequest {
    /// Reads an HTTP Request from the stream
    /// Only GET methods are supported
    /// HTTP version must be 1.1
    /// Valid URIs are "/", "/data", "/resources/css/*", "/resources/image/*" and "/favicon.ico"
    pub fn read_from<T: io::Read>(mut stream: T) -> HttpResult<HttpRequest> {
        let mut buff = [0u8; 1024];
        let size = stream.read(&mut buff)?;
        let tmp = String::from_utf8_lossy(&buff[..size]).replace("\r", "");
        let mut lines = tmp.lines();
        let mut request_line = lines
            .next()
            .ok_or("Request tiene formato incorrecto")?
            .split_whitespace();

        let method = request_line.next().ok_or("No se pudo obtener método")?;
        HttpRequest::validate_method(method)?;

        let request_uri = request_line.next().ok_or("No se pudo obtener URI")?;
        let request = Request::try_from(request_uri)?;

        let http_version = request_line
            .next()
            .ok_or("No se pudo obtener versión HTTP")?;
        let version = HttpRequest::parse_http_version(http_version)?;

        Ok(HttpRequest {
            request,
            version,
            headers: HttpRequest::parse_headers(&mut lines),
            body: HttpRequest::parse_body(&mut lines),
        })
    }

    fn validate_method(method: &str) -> HttpResult<()> {
        if method != GET_METHOD {
            return Err(format!("Solo se soporta el método {}", GET_METHOD).into());
        }
        Ok(())
    }

    fn parse_headers(lines: &mut Lines) -> Vec<String> {
        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            headers.push(line.to_string());
        }
        headers
    }

    fn parse_body(lines: &mut Lines) -> Vec<String> {
        let mut body = Vec::new();
        for line in lines {
            body.push(line.to_string());
        }
        body
    }

    fn parse_http_version(http_version: &str) -> HttpResult<HttpVersion> {
        match HttpVersion::try_from(http_version) {
            Ok(HttpVersion::V1_1) => Ok(HttpVersion::V1_1),
            _ => Err(format!("Http version not supported: {}", http_version).into()),
        }
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn version(&self) -> &HttpVersion {
        &self.version
    }

    pub fn headers(&self) -> &Vec<String> {
        &self.headers
    }

    pub fn body(&self) -> &Vec<String> {
        &self.body
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
        let response = format!(
            "{} {} {}\r\nContent-Length: {}\r\n",
            String::from(self.version),
            self.code.reason_phrase(),
            String::from(&self.code),
            self.body_len()
        );

        stream.write_all(response.as_bytes())?;

        let headers = self.headers.as_deref().unwrap_or_else(|| "".as_bytes());
        let body = self.body.as_deref().unwrap_or(&[]);
        stream.write_all(headers)?;
        stream.write_all("\r\n".as_bytes())?;

        stream.write_all(body)?;

        stream.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::HttpVersion;

    use super::{HttpRequest, Request};

    #[test]
    fn empty_request_invalid() {
        let request = HttpRequest::read_from("".as_bytes());
        assert!(request.is_err());
    }

    #[test]
    fn post_method_unsopported() {
        let request = HttpRequest::read_from(
            "POST / HTTP/1.1\r\n
        Host: foo.com\r\n
        Content-Type: application/x-www-form-urlencoded\r\n
        Content-Length: 13\r\n
        \r\n
        say=Hi&to=Mom"
                .as_bytes(),
        );
        assert!(request.is_err());
    }

    #[test]
    fn invalid_uri() {
        let request = HttpRequest::read_from("GET /index.html HTTP/1.1\r\n".as_bytes());
        assert!(request.is_err());
    }

    #[test]
    fn valid_request_empty() {
        let request = HttpRequest::read_from("GET /favicon.ico HTTP/1.1\r\n".as_bytes());

        assert!(request.is_ok());
        let request = request.unwrap();
        assert!(matches!(request.request(), Request::Favicon));
        assert!(matches!(request.version(), HttpVersion::V1_1));
        assert!(request.headers().is_empty());
        assert!(request.body().is_empty());
    }

    #[test]
    fn valid_request_with_headers() {
        let request = HttpRequest::read_from("GET / HTTP/1.1\r\nsoy un header\r\n:D".as_bytes());

        assert!(request.is_ok());
        let request = request.unwrap();
        assert!(matches!(request.request(), Request::Index));
        assert!(matches!(request.version(), HttpVersion::V1_1));
        let headers = request.headers();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], "soy un header");
        assert_eq!(headers[1], ":D");
        assert!(request.body().is_empty());
    }

    #[test]
    fn valid_request_with_headers_and_body() {
        let request = HttpRequest::read_from(
            "GET /resources/image/foto HTTP/1.1\r\nheader\r\n\r\nbody".as_bytes(),
        );

        assert!(request.is_ok());
        let request = request.unwrap();
        if let Request::Image(ref resource) = request.request() {
            assert_eq!(resource, "foto");
        } else {
            panic!("Request incorrecto");
        }
        assert!(matches!(request.version(), HttpVersion::V1_1));
        let headers = request.headers();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "header");
        let body = request.body();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0], "body");
    }
}
