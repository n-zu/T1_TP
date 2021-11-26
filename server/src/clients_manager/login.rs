use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use tracing::debug;

use crate::server::{server_error::ServerErrorKind, ServerError, ServerResult};

pub fn search_password(accounts_path: &str, user_name_to_search: &str) -> ServerResult<String> {
    let file = BufReader::new(File::open(accounts_path)?);
    search_in_file(file, user_name_to_search)
}

pub fn search_in_file(mut file: impl BufRead, user_name_to_search: &str) -> ServerResult<String> {
    let mut buf = String::new();
    while file.read_line(&mut buf).is_ok() {
        if buf.is_empty() {
            break;
        }
        let (user_name, password) = parse_account_line(buf.trim())?;
        if user_name == user_name_to_search {
            return Ok(password.to_owned());
        }
        buf.clear();
    }
    Err(ServerError::new_kind(
        &format!(
            "Cliente con usuario {} no se encontro en el archivo de cuentas",
            user_name_to_search
        ),
        ServerErrorKind::ClientNotInWhitelist,
    ))
}

fn parse_account_line(line: &str) -> ServerResult<(&str, &str)> {
    let tokens: Vec<&str> = line.split(',').collect();
    if tokens.len() != 2 {
        debug!("{:?}", tokens);
        return Err(ServerError::new_msg(
            "Formato de archivo de cuentas invalido",
        ));
    }
    let user_name = tokens[0];
    let password = tokens[1];
    Ok((user_name, password))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, Cursor};

    use crate::server::server_error::ServerErrorKind;

    use super::search_in_file;

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
        let password = search_in_file(cursor, "efoppiano").unwrap();
        assert_eq!(password, "105836");
    }

    #[test]
    fn test_search_last_account() {
        let cursor = valid_accounts_file();
        let password = search_in_file(cursor, "fdelu").unwrap();
        assert_eq!(password, "fdelu");
    }

    #[test]
    fn test_username_not_in_file() {
        let cursor = valid_accounts_file();
        let password = search_in_file(cursor, "NoExiste");
        assert_eq!(
            password.err().unwrap().kind(),
            ServerErrorKind::ClientNotInWhitelist
        );
    }

    #[test]
    fn test_invalid_file_format() {
        let cursor = invalid_accounts_file();
        let password = search_in_file(cursor, "FacuMastri");
        assert!(password.is_err());
    }
}
