//! The saved servers as the menus show them and change them on disk: `servers.ron` in the game
//! folder, read in file order and written at once, atomically, after every change. A file that
//! cannot be read is never written, so the player may still repair it.

use std::fs;
use std::io;
use std::path::Path;

use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::network::address::{parse_server_address, ServerAddress};

/// A saved server: its name and its address as typed, trimmed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ServerItem {
    pub name: String,
    pub ip: String,
}

/// The server of this computer on the default port, which a missing list offers.
pub fn localhost() -> ServerItem {
    ServerItem {
        name: "localhost".to_string(),
        ip: "127.0.0.1:8000".to_string(),
    }
}

/// The saved servers in file order. Missing: `[localhost()]`, nothing written. Empty: `[]`.
/// Unreadable: `Err` of kind `InvalidData`.
pub fn read_servers(path: &Path) -> io::Result<Vec<ServerItem>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(vec![localhost()]),
        Err(err) => return Err(err),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    ron::from_str(&text).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// Writes `servers` to `path` in the format the list always had, through `<path>.tmp` renamed
/// over it, so that a failed write leaves the previous list.
pub(super) fn write_servers(path: &Path, servers: &[ServerItem]) -> io::Result<()> {
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);
    let data = ron::ser::to_string_pretty(&servers, pretty_config).map_err(io::Error::other)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

/// The position in `servers` of the server `name`, whatever its case.
fn position(servers: &[ServerItem], name: &str) -> io::Result<usize> {
    servers
        .iter()
        .position(|server| server.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no server {name:?}")))
}

/// Appends `server` to the list of `path`.
#[allow(dead_code)] // The Add server dialog, Task 5
pub fn add_server(path: &Path, server: ServerItem) -> io::Result<()> {
    let mut servers = read_servers(path)?;
    servers.push(server);
    write_servers(path, &servers)
}

/// Replaces the server `name` of the list of `path`, whatever its case, by `server` in its place.
#[allow(dead_code)] // The Edit dialog, Task 7
pub fn edit_server(path: &Path, name: &str, server: ServerItem) -> io::Result<()> {
    let mut servers = read_servers(path)?;
    let index = position(&servers, name)?;
    servers[index] = server;
    write_servers(path, &servers)
}

/// Removes the server `name`, whatever its case, from the list of `path`.
#[allow(dead_code)] // The Multiplayer screen, Task 6
pub fn remove_server(path: &Path, name: &str) -> io::Result<()> {
    let mut servers = read_servers(path)?;
    let index = position(&servers, name)?;
    servers.remove(index);
    write_servers(path, &servers)
}

/// The most characters of a server name.
pub const MAX_SERVER_NAME_LENGTH: usize = 32;
/// Shown under the name field when it is empty.
pub const EMPTY_SERVER_NAME: &str = "Enter a name";
/// Shown under the name field when the name is too long or has a character the font lacks.
pub const INVALID_SERVER_NAME: &str = "Use letters, digits and punctuation";
/// Shown under the name field when another server has the typed name, whatever its case.
pub const SERVER_NAME_EXISTS: &str = "Another server has this name";
/// Shown under the address field when another server has the typed address.
pub const SERVER_EXISTS: &str = "Another server has this address";

/// The trimmed name, or the message to show. `others` are the names of the other servers.
/// A name is 1 to 32 printable ASCII characters but the backtick, which the font lacks.
#[allow(dead_code)] // The Add server dialog, Task 5
pub fn check_server_name(typed: &str, others: &[String]) -> Result<String, &'static str> {
    let name = typed.trim();
    if name.is_empty() {
        return Err(EMPTY_SERVER_NAME);
    }
    if name.len() > MAX_SERVER_NAME_LENGTH
        || !name.chars().all(|c| matches!(c, ' '..='~') && c != '`')
    {
        return Err(INVALID_SERVER_NAME);
    }
    if others.iter().any(|other| other.eq_ignore_ascii_case(name)) {
        return Err(SERVER_NAME_EXISTS);
    }
    Ok(name.to_string())
}

/// The parsed address, or the message to show. `others` are the other servers; those whose
/// address does not parse cannot be the same server.
#[allow(dead_code)] // The Add server dialog, Task 5
pub fn check_server_address(
    typed: &str,
    others: &[ServerItem],
) -> Result<ServerAddress, &'static str> {
    let address = parse_server_address(typed)?;
    let key = address.key();
    let same = |other: &ServerItem| parse_server_address(&other.ip).is_ok_and(|a| a.key() == key);
    if others.iter().any(same) {
        return Err(SERVER_EXISTS);
    }
    Ok(address)
}

/// The first of "My server", "My server 2", "My server 3"... that none of `others` has,
/// whatever its case.
#[allow(dead_code)] // The Add server dialog, Task 5
pub fn free_server_name(others: &[String]) -> String {
    let taken = |name: &str| others.iter().any(|other| other.eq_ignore_ascii_case(name));
    std::iter::once("My server".to_string())
        .chain((2..).map(|index| format!("My server {index}")))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::SERVER_LIST_SAVE_NAME;
    use crate::network::address::INVALID_ADDRESS;

    fn server(name: &str, ip: &str) -> ServerItem {
        ServerItem {
            name: name.to_string(),
            ip: ip.to_string(),
        }
    }

    fn names(servers: &[ServerItem]) -> Vec<&str> {
        servers.iter().map(|server| server.name.as_str()).collect()
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_missing_list_offers_localhost_and_writes_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        assert_eq!(read_servers(&path).unwrap(), [localhost()]);
        assert_eq!(
            localhost(),
            server("localhost", "127.0.0.1:8000"),
            "the local server on the default port"
        );
        assert!(!path.exists());
    }

    #[test]
    fn an_empty_list_is_empty_and_an_unreadable_one_is_an_error() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        for empty in ["", " \n", "[]"] {
            fs::write(&path, empty).unwrap();
            assert_eq!(read_servers(&path).unwrap(), [], "{empty:?}");
        }
        fs::write(&path, "not ron").unwrap();
        assert_eq!(
            read_servers(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn an_old_server_list_still_loads() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        let old = "[\n    /*[0]*/ (\n        name: \"localhost\",\n        ip: \"127.0.0.1:8000\",\n    ),\n    /*[1]*/ (\n        name: \"Friends\",\n        ip: \"192.168.1.20:8000\",\n    ),\n]";
        fs::write(&path, old).unwrap();
        assert_eq!(
            read_servers(&path).unwrap(),
            [
                server("localhost", "127.0.0.1:8000"),
                server("Friends", "192.168.1.20:8000")
            ]
        );
    }

    #[test]
    fn adding_editing_and_removing_write_the_list_at_once_in_order() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        write_servers(
            &path,
            &[server("zulu", "10.0.0.1"), server("alpha", "10.0.0.2")],
        )
        .unwrap();

        add_server(&path, server("bravo", "10.0.0.3")).unwrap();
        assert_eq!(
            names(&read_servers(&path).unwrap()),
            ["zulu", "alpha", "bravo"]
        );

        edit_server(&path, "ALPHA", server("Alpha2", "10.0.0.9")).unwrap();
        let servers = read_servers(&path).unwrap();
        assert_eq!(names(&servers), ["zulu", "Alpha2", "bravo"]);
        assert_eq!(servers[1].ip, "10.0.0.9");

        remove_server(&path, "zulu").unwrap();
        assert_eq!(names(&read_servers(&path).unwrap()), ["Alpha2", "bravo"]);

        assert!(!folder.path().join("servers.ron.tmp").exists());
        assert_eq!(
            remove_server(&path, "nobody").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            edit_server(&path, "nobody", server("x", "10.0.0.4"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(names(&read_servers(&path).unwrap()), ["Alpha2", "bravo"]);
    }

    #[test]
    fn a_removed_localhost_stays_removed() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        remove_server(&path, "localhost").unwrap();
        assert_eq!(read_servers(&path).unwrap(), []);
    }

    #[test]
    fn an_unreadable_list_is_never_written() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(SERVER_LIST_SAVE_NAME);
        fs::write(&path, "not ron").unwrap();
        let results = [
            add_server(&path, server("bravo", "10.0.0.3")),
            edit_server(&path, "bravo", server("bravo", "10.0.0.4")),
            remove_server(&path, "bravo"),
        ];
        for result in results {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), "not ron");
    }

    #[test]
    fn server_names_are_checked_and_trimmed() {
        let others = strings(&["Home"]);
        assert_eq!(check_server_name(" Base ", &others), Ok("Base".to_string()));
        for empty in ["", "  "] {
            assert_eq!(
                check_server_name(empty, &others),
                Err(EMPTY_SERVER_NAME),
                "{empty:?}"
            );
        }
        assert_eq!(check_server_name("home ", &others), Err(SERVER_NAME_EXISTS));
        for invalid in [&"x".repeat(33), "a`b", "é"] {
            assert_eq!(
                check_server_name(invalid, &others),
                Err(INVALID_SERVER_NAME),
                "{invalid:?}"
            );
        }
        let longest = "x".repeat(MAX_SERVER_NAME_LENGTH);
        assert_eq!(check_server_name(&longest, &others), Ok(longest.clone()));
    }

    #[test]
    fn a_server_address_is_checked_against_the_others() {
        let others = [
            server("Friends", "192.168.1.20:8000"),
            server("Old", "garbage"),
            server("Broken", "bad host"),
        ];
        assert_eq!(
            check_server_address("192.168.1.20", &others),
            Err(SERVER_EXISTS)
        );
        assert_eq!(
            check_server_address("192.168.1.20:8001", &others).map(|a| a.to_string()),
            Ok("192.168.1.20:8001".to_string())
        );
        assert_eq!(
            check_server_address("bad host", &others),
            Err(INVALID_ADDRESS)
        );
    }

    #[test]
    fn a_free_server_name_is_offered() {
        assert_eq!(free_server_name(&[]), "My server");
        assert_eq!(free_server_name(&strings(&["my server"])), "My server 2");
        assert_eq!(
            free_server_name(&strings(&["My server", "My server 2"])),
            "My server 3"
        );
    }
}
