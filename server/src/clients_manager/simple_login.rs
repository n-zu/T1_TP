use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{self, BufRead, BufReader},
};

use packets::traits::{Login, LoginResult};

const CACHE_SIZE: usize = 128;
#[doc(hidden)]
const SEP: &str = ",";

type Username = String;
type Password = String;

/// Basic login for the server.
/// Reads usernames and passwords from
/// a plain text file whose path is *path*, and
/// the format is 'username,password'.
///
/// The first [`CACHE_SIZE`] entries in the file are
/// stored in memory to speed up the connection on servers
/// with few users.
#[derive(Debug)]
pub struct SimpleLogin {
    /// Username and Password cache
    cache: HashMap<Username, Password>,
    /// Path of the file from which usernames and passwords
    /// are read. It is [None] if the entire file could be
    /// loaded into memory (that is, the number of users was
    /// less than or equal to [CACHE_SIZE])
    path: Option<String>,
}

impl std::error::Error for SimpleLogin {}

impl Login for SimpleLogin {
    fn login(&mut self, user_name: &str, password: &str) -> io::Result<LoginResult> {
        if let Some(real_password) = self.cache.get(user_name) {
            if password == real_password {
                Ok(LoginResult::Accepted)
            } else {
                Ok(LoginResult::InvalidPassword)
            }
        } else {
            self.search_in_file(user_name, password)
        }
    }
}

impl SimpleLogin {
    pub fn new(path: &str) -> io::Result<Self> {
        let reader = BufReader::new(File::open(path)?);
        SimpleLogin::new_from_stream(reader, path)
    }

    fn new_from_stream(mut stream: impl BufRead, path: &str) -> io::Result<Self> {
        let mut cache = HashMap::with_capacity(CACHE_SIZE);
        let mut path = Some(path.to_string());

        for _ in 0..CACHE_SIZE {
            let mut buf = String::new();

            if stream.read_line(&mut buf)? == 0 {
                path.take();
                break;
            }
            let (user_name, password) = buf.trim().split_once(SEP).ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "Formato de archivo invalido",
            ))?;
            if cache
                .insert(user_name.to_string(), password.to_string())
                .is_some()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Usuario duplicado",
                ));
            }
        }

        Ok(Self { cache, path })
    }

    fn search_in_file(&mut self, user_name: &str, password: &str) -> io::Result<LoginResult> {
        let path = match &self.path {
            Some(path) => path,
            None => return Ok(LoginResult::UsernameNotFound),
        };

        let mut reader = BufReader::new(File::open(path)?);
        let mut buf = String::new();
        while reader.read_line(&mut buf)? != 0 {
            let (found_user_name, found_password) = buf.trim().split_once(SEP).ok_or(
                io::Error::new(io::ErrorKind::InvalidData, "Formato de archivo invalido"),
            )?;
            if found_user_name == user_name {
                if found_password == password {
                    return Ok(LoginResult::Accepted);
                } else {
                    return Ok(LoginResult::InvalidPassword);
                }
            }
            buf.clear();
        }
        Ok(LoginResult::UsernameNotFound)
    }
}

impl fmt::Display for SimpleLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl SimpleLogin {}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, Cursor};

    use packets::traits::{LoginResult, Login};

    use crate::{
        clients_manager::simple_login::SimpleLogin,
    };

    fn valid_accounts_file() -> impl BufRead {
        Cursor::new(
            "efoppiano,105836
FacuMastri,55555
NicolasZulaica,password
fdelu,fdelu",
        )
    }

    fn invalid_accounts_file() -> impl BufRead {
        Cursor::new(
            "efoppiano,105836
FacuMastri;55555
NicolasZulaica,password
fdelu,fdelu",
        )
    }

    #[test]
    fn test_search_first_account() {
        let cursor = valid_accounts_file();
        let mut login = SimpleLogin::new_from_stream(cursor, "").unwrap();

        let result = login.login("efoppiano", "105836").unwrap();
        assert_eq!(result, LoginResult::Accepted);
    }

    #[test]
    fn test_search_last_account() {
        let cursor = valid_accounts_file();
        let mut login = SimpleLogin::new_from_stream(cursor, "").unwrap();

        let result = login.login("fdelu", "fdelu").unwrap();
        assert_eq!(result, LoginResult::Accepted);
    }

    #[test]
    fn test_invalid_password() {
        let cursor = valid_accounts_file();
        let mut login = SimpleLogin::new_from_stream(cursor, "").unwrap();

        let result = login.login("FacuMastri", "ContraseñaInvalida").unwrap();
        assert_eq!(result, LoginResult::InvalidPassword);
    }

    #[test]
    fn test_username_not_in_file() {
        let cursor = valid_accounts_file();
        let mut login = SimpleLogin::new_from_stream(cursor, "").unwrap();

        let result = login.login("NoExiste", "password").unwrap();
        assert_eq!(result, LoginResult::UsernameNotFound);
    }

    #[test]
    fn test_invalid_file_format() {
        let cursor = invalid_accounts_file();
        let error = SimpleLogin::new_from_stream(cursor, "").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
