use std::time::Duration;

use packets::{
    connack::ConnackReturnCode,
    connect::{ConnectBuilder, LastWill},
    publish::Publish,
    qos::QoSLevel,
};

use crate::{
    clients_manager::{simple_login::SimpleLogin, ConnectInfo},
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientIdArg, ServerResult},
    test_helpers::iomock::IOMock,
};

use super::{ClientsManager, GENERIC_ID_SUFFIX};

fn make_manager_with_clients(
    ids: Vec<&ClientIdArg>,
    clean_session: bool,
    accounts_path: Option<&str>,
) -> ServerResult<ClientsManager<IOMock, u16>> {
    let mut manager = match accounts_path {
        Some(path) => ClientsManager::new(Some(Box::new(SimpleLogin::new(path).unwrap()))),
        None => ClientsManager::new(None),
    };
    for (i, id) in ids.into_iter().enumerate() {
        let iomock = IOMock::new();
        let connect = ConnectBuilder::new(id, 0, clean_session)
            .unwrap()
            .build()
            .unwrap();
        let network_connection = NetworkConnection::new(i as u16, iomock);
        manager.new_session(network_connection, connect)?;
    }
    Ok(manager)
}

#[test]
fn test_creation() {
    let manager = ClientsManager::<IOMock, u16>::new(None);
    assert!(manager.clients.is_empty());
    assert_eq!(manager.generic_ids_counter, 0);
    assert!(manager.clients.is_empty());
}

#[test]
fn test_new_session_with_id() {
    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    let iomock = IOMock::new();
    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .build()
        .unwrap();
    let network_connection = NetworkConnection::new(0, iomock);
    let connect_info = manager.new_session(network_connection, connect).unwrap();

    let expected = ConnectInfo {
        id: String::from("client_id"),
        session_present: false,
        takeover_last_will: None,
    };

    assert!(manager.clients.contains_key("client_id"));
    assert_eq!(connect_info, expected);
}

#[test]
fn test_new_session_empty_id() {
    let manager = make_manager_with_clients(vec![""], true, None).unwrap();

    assert!(manager.clients.contains_key("__CLIENT__1"));
}

#[test]
fn test_new_session_empty_id_does_not_work_with_clean_session_true() {
    let manager = make_manager_with_clients(vec![""], false, None);

    assert_eq!(
        manager.unwrap_err().kind(),
        ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected)
    );
}

#[test]
fn test_multiple_sessions_with_id() {
    let manager = make_manager_with_clients(vec!["client_id1", "client_id2"], true, None).unwrap();

    assert!(manager.clients.contains_key("client_id1"));
    assert!(manager.clients.contains_key("client_id2"));
}

#[test]
fn test_multiple_sessions_empty_id() {
    let manager = make_manager_with_clients(vec!["", ""], true, None).unwrap();

    assert!(manager.clients.contains_key("__CLIENT__1"));
    assert!(manager.clients.contains_key("__CLIENT__2"));
}

#[test]
fn test_authorized_session() {
    let iomock = IOMock::new();
    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    let network_connection = NetworkConnection::new(0, iomock);
    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));

    manager.new_session(network_connection, connect).unwrap();
    assert!(manager.clients.contains_key("client_id"));
}

#[test]
fn test_invalid_username_should_fail() {
    let iomock = IOMock::new();
    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("usuario invalido")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    let network_connection = NetworkConnection::new(0, iomock);
    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));

    let result = manager.new_session(network_connection, connect);
    assert_eq!(
        result.unwrap_err().kind(),
        ServerErrorKind::ConnectionRefused(ConnackReturnCode::NotAuthorized)
    );
}

#[test]
fn test_invalid_password_should_fail() {
    let iomock = IOMock::new();
    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("contraseña invalida")
        .unwrap()
        .build()
        .unwrap();
    let network_connection = NetworkConnection::new(0, iomock);
    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));

    let result = manager.new_session(network_connection, connect);
    assert_eq!(
        result.unwrap_err().kind(),
        ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword)
    );
}

#[test]
fn test_ids_collision_should_fail() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    let connect_2 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("foo")
        .unwrap()
        .password("bar")
        .unwrap()
        .build()
        .unwrap();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_2 = NetworkConnection::new(1, iomock_2);

    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();
    let result = manager.new_session(network_connection_2, connect_2);
    assert_eq!(
        result.unwrap_err().kind(),
        ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected)
    );
}

#[test]
fn test_connect_with_forbidden_id_should_fail() {
    let result = make_manager_with_clients(vec![GENERIC_ID_SUFFIX], true, None);
    assert_eq!(
        result.unwrap_err().kind(),
        ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected)
    );
}

#[test]
fn test_takeover() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    let connect_2 = connect_1.clone();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_2 = NetworkConnection::new(1, iomock_2);

    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();

    let connect_info = manager
        .new_session(network_connection_2, connect_2)
        .unwrap();
    let expected = ConnectInfo {
        id: String::from("client_id"),
        session_present: true,
        takeover_last_will: None,
    };
    assert_eq!(connect_info, expected);
}

#[test]
fn test_takeover_should_update_client_properties() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    let connect_2 = ConnectBuilder::new("client_id", 15, true)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_2 = NetworkConnection::new(1, iomock_2);

    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();
    manager
        .new_session(network_connection_2, connect_2)
        .unwrap();

    let keep_alive = manager
        .client_do("client_id", |client| Ok(client.keep_alive()))
        .unwrap();
    let clean_session = manager
        .client_do("client_id", |client| Ok(client.clean_session()))
        .unwrap();

    assert_eq!(keep_alive, Some(Duration::from_millis(22500)));
    assert!(clean_session);
}

#[test]
fn test_takeover_should_return_lastwill() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .user_name("user")
        .unwrap()
        .password("pass")
        .unwrap()
        .last_will(LastWill::new(
            String::from("top"),
            String::from("message"),
            QoSLevel::QoSLevel0,
            false,
        ))
        .build()
        .unwrap();
    let connect_2 = connect_1.clone();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_2 = NetworkConnection::new(1, iomock_2);

    let mut manager = ClientsManager::<IOMock, u16>::new(Some(Box::new(
        SimpleLogin::new("tests/files/test_accounts.csv").unwrap(),
    )));
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();
    let connect_info = manager
        .new_session(network_connection_2, connect_2)
        .unwrap();

    let publish_expected =
        Publish::new(false, QoSLevel::QoSLevel0, false, "top", "message", None).unwrap();
    assert_eq!(connect_info.takeover_last_will.unwrap(), publish_expected);
}

#[test]
fn test_disconnect_should_prevent_double_disconnection() {
    // Esta situacion se da principalmente en un takeover: un
    // thread con la nueva conexion TCP desconecta al cliente,
    // el thread que manejaba la conexion recibe un EOF en el
    // TcpStream, e intenta desconectarlo
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, true)
        .unwrap()
        .build()
        .unwrap();

    let connect_2 = connect_1.clone();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_copy = network_connection_1.try_clone().unwrap();
    let network_connection_2 = NetworkConnection::new(1, iomock_2);

    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();
    manager
        .new_session(network_connection_2, connect_2)
        .unwrap();

    // El metodo no falla, porque la situacion de doble desconexion
    // ocurre naturalmente en el takeover
    manager
        .disconnect("client_id", network_connection_copy, true)
        .unwrap();

    // Si el disconnect hubiera desconectado al cliente, ya no estaria en
    // el HashMap de clientes, porque se conecto con clean_session en true
    assert!(manager.clients.contains_key("client_id"));
}

#[test]
fn test_disconnect_gracefully_should_not_return_last_will() {
    let iomock = IOMock::new();

    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .last_will(LastWill::new(
            String::from("top"),
            String::from("message"),
            QoSLevel::QoSLevel0,
            false,
        ))
        .build()
        .unwrap();

    let network_connection = NetworkConnection::new(0, iomock);
    let network_connection_copy = network_connection.try_clone().unwrap();

    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    manager.new_session(network_connection, connect).unwrap();

    let disconnect_info = manager
        .disconnect("client_id", network_connection_copy, true)
        .unwrap();

    assert!(disconnect_info.publish_last_will.is_none());
}

#[test]
fn test_disconnect_ungracefully_should_return_last_will() {
    let iomock = IOMock::new();

    let connect = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .last_will(LastWill::new(
            String::from("top"),
            String::from("message"),
            QoSLevel::QoSLevel0,
            false,
        ))
        .build()
        .unwrap();

    let network_connection = NetworkConnection::new(0, iomock);
    let network_connection_copy = network_connection.try_clone().unwrap();

    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    manager.new_session(network_connection, connect).unwrap();

    let disconnect_info = manager
        .disconnect("client_id", network_connection_copy, false)
        .unwrap();
    let expected = Publish::new(false, QoSLevel::QoSLevel0, false, "top", "message", None).unwrap();

    assert_eq!(disconnect_info.publish_last_will.unwrap(), expected);
}

#[test]
fn test_disconnect_persistent_session_should_be_remembered() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, false)
        .unwrap()
        .build()
        .unwrap();

    let connect_2 = connect_1.clone();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_copy = network_connection_1.try_clone().unwrap();

    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();

    manager
        .disconnect("client_id", network_connection_copy, true)
        .unwrap();

    let network_connection_1 = NetworkConnection::new(0, iomock_2);
    let connect_info = manager
        .new_session(network_connection_1, connect_2)
        .unwrap();

    assert!(connect_info.session_present);
}

#[test]
fn test_disconnect_non_persistent_session_should_not_be_remembered() {
    let iomock_1 = IOMock::new();
    let iomock_2 = IOMock::new();

    let connect_1 = ConnectBuilder::new("client_id", 0, true)
        .unwrap()
        .build()
        .unwrap();

    let connect_2 = connect_1.clone();

    let network_connection_1 = NetworkConnection::new(0, iomock_1);
    let network_connection_copy = network_connection_1.try_clone().unwrap();

    let mut manager = ClientsManager::<IOMock, u16>::new(None);
    manager
        .new_session(network_connection_1, connect_1)
        .unwrap();

    manager
        .disconnect("client_id", network_connection_copy, true)
        .unwrap();

    let network_connection_1 = NetworkConnection::new(0, iomock_2);
    let connect_info = manager
        .new_session(network_connection_1, connect_2)
        .unwrap();

    assert!(!connect_info.session_present);
}
