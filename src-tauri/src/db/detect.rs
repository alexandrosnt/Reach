//! Finding the databases on a machine Reach already has a terminal open to.
//!
//! Three sources, best first. `ss` lists listening sockets and, for the
//! user's own processes, which program owns each. `docker ps` names the
//! image behind a published port. And when neither is available — no `ss`,
//! a locked-down container host — the well-known ports are simply tried
//! through the SSH connection itself. Nothing here needs root, and nothing
//! reads a password: the user supplies that once, and it goes to the vault.

use serde::Serialize;

use super::types::Engine;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub engine: Engine,
    /// As seen from the SSH server: 127.0.0.1 for anything on loopback or
    /// every interface, otherwise the address it is bound to.
    pub host: String,
    pub port: u16,
    /// A container or program name, to tell two PostgreSQLs apart.
    pub label: Option<String>,
}

/// Ports worth trying when nothing better is known.
pub const WELL_KNOWN: [(u16, Engine); 6] = [
    (5432, Engine::Postgres),
    (5433, Engine::Postgres),
    (3306, Engine::Mysql),
    (3307, Engine::Mysql),
    (1433, Engine::Mssql),
    (6379, Engine::Redis),
];

/// The one command run over SSH. Each section is optional; a missing tool
/// leaves its section empty rather than failing the whole probe.
pub const PROBE_SCRIPT: &str = "ss -Hltnp 2>/dev/null; echo '@@DOCKER'; \
docker ps --format '{{.Names}}\t{{.Image}}\t{{.Ports}}' 2>/dev/null; echo '@@END'";

fn engine_from_name(name: &str) -> Option<Engine> {
    let n = name.to_lowercase();
    if n.contains("mariadb") {
        Some(Engine::Mariadb)
    } else if n.contains("mysql") || n.contains("percona") {
        Some(Engine::Mysql)
    } else if n.contains("postgres") || n.contains("postgis") || n.contains("timescale") || n.contains("supabase") {
        Some(Engine::Postgres)
    } else if n.contains("sqlservr") || n.contains("mssql") || n.contains("sql-server") || n.contains("azure-sql-edge") {
        Some(Engine::Mssql)
    } else if n.contains("redis") || n.contains("valkey") || n.contains("keydb") {
        Some(Engine::Redis)
    } else {
        None
    }
}

fn engine_from_port(port: u16) -> Option<Engine> {
    WELL_KNOWN.iter().find(|(p, _)| *p == port).map(|(_, e)| *e)
}

/// `127.0.0.1:5432`, `[::]:3306`, `*:6379`, `0.0.0.0%lo:5432` → (host, port).
fn split_addr(addr: &str) -> Option<(String, u16)> {
    let (host, port) = addr.rsplit_once(':')?;
    let port = port.parse().ok()?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = host.split('%').next().unwrap_or(host);
    let reachable = match host {
        "*" | "0.0.0.0" | "::" | "127.0.0.1" | "::1" | "localhost" => "127.0.0.1".to_string(),
        h if h.starts_with("127.") => h.to_string(),
        h => h.to_string(),
    };
    Some((reachable, port))
}

/// Parse `ss -Hltnp` and the Docker section of [`PROBE_SCRIPT`]'s output.
pub fn parse_probe(output: &str) -> Vec<Detected> {
    let mut out: Vec<Detected> = Vec::new();
    let (ss, rest) = output.split_once("@@DOCKER").unwrap_or((output, ""));
    let docker = rest.split("@@END").next().unwrap_or("");

    // Docker first: its labels are the most specific.
    for line in docker.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.split('\t');
        let (Some(name), Some(image), Some(ports)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let by_image = engine_from_name(image);
        for mapping in ports.split(", ") {
            // 0.0.0.0:5433->5432/tcp. Unpublished ports have no "->".
            let Some((published, inner)) = mapping.split_once("->") else { continue };
            let Some((host, port)) = split_addr(published) else { continue };
            let inner_port = inner.split('/').next().and_then(|p| p.parse().ok());
            let Some(engine) = by_image.or_else(|| inner_port.and_then(engine_from_port)) else { continue };
            push(&mut out, Detected { engine, host, port, label: Some(name.to_string()) });
        }
    }

    for line in ss.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // State Recv-Q Send-Q Local Peer [Process]
        let Some(local) = cols.get(3) else { continue };
        let Some((host, port)) = split_addr(local) else { continue };
        let process = cols
            .get(5..)
            .map(|p| p.join(" "))
            .and_then(|p| p.split("((\"").nth(1).map(|s| s.split('"').next().unwrap_or("").to_string()));
        let engine = process.as_deref().and_then(engine_from_name).or_else(|| engine_from_port(port));
        if let Some(engine) = engine {
            push(&mut out, Detected { engine, host, port, label: process });
        }
    }
    out
}

/// Keep the first sighting of each host:port; a Docker label beats a bare
/// port, and an IPv4 and IPv6 listener on the same port are one database.
fn push(out: &mut Vec<Detected>, d: Detected) {
    if !out.iter().any(|x| x.port == d.port && x.host == d.host) {
        out.push(d);
    }
}

/// A MySQL server greets first; MariaDB says so in its version string.
pub fn refine_mysql(greeting: &[u8]) -> Engine {
    if String::from_utf8_lossy(greeting).to_lowercase().contains("mariadb") {
        Engine::Mariadb
    } else {
        Engine::Mysql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ss_with_and_without_process_names() {
        let out = "LISTEN 0 244 127.0.0.1:5432 0.0.0.0:* users:((\"postgres\",pid=812,fd=5))\n\
                   LISTEN 0 80 [::]:3306 [::]:*\n\
                   LISTEN 0 128 0.0.0.0:22 0.0.0.0:* \n\
                   LISTEN 0 511 10.0.0.5:6380 0.0.0.0:* users:((\"redis-server\",pid=9,fd=6))\n@@DOCKER\n@@END";
        let found = parse_probe(out);
        assert_eq!(
            found,
            vec![
                Detected { engine: Engine::Postgres, host: "127.0.0.1".into(), port: 5432, label: Some("postgres".into()) },
                Detected { engine: Engine::Mysql, host: "127.0.0.1".into(), port: 3306, label: None },
                Detected { engine: Engine::Redis, host: "10.0.0.5".into(), port: 6380, label: Some("redis-server".into()) },
            ]
        );
    }

    #[test]
    fn docker_names_the_container_and_its_published_port() {
        let out = "LISTEN 0 4096 0.0.0.0:5433 0.0.0.0:*\n@@DOCKER\n\
                   shop-db\tpostgres:16\t0.0.0.0:5433->5432/tcp, :::5433->5432/tcp\n\
                   cache\tbitnami/valkey\t6379/tcp\n\
                   legacy\tmariadb:10.11\t127.0.0.1:3307->3306/tcp\n@@END";
        let found = parse_probe(out);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0], Detected { engine: Engine::Postgres, host: "127.0.0.1".into(), port: 5433, label: Some("shop-db".into()) });
        assert_eq!(found[1].engine, Engine::Mariadb);
        assert_eq!(found[1].port, 3307);
    }

    #[test]
    fn a_program_on_an_odd_port_is_still_recognised() {
        let found = parse_probe("LISTEN 0 1 127.0.0.1:15432 0.0.0.0:* users:((\"postgres\",pid=1,fd=3))\n@@DOCKER\n@@END");
        assert_eq!(found[0].engine, Engine::Postgres);
        assert_eq!(found[0].port, 15432);
    }

    #[test]
    fn mariadb_is_told_apart_by_its_greeting() {
        assert_eq!(refine_mysql(b"\x0a5.5.5-10.11.6-MariaDB-0+deb12u1\0"), Engine::Mariadb);
        assert_eq!(refine_mysql(b"\x0a8.0.36\0"), Engine::Mysql);
    }
}
