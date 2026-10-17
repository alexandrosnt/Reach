use super::*;

fn utf16(s: &str, bom: bool) -> Vec<u8> {
    let mut out = if bom { vec![0xFF, 0xFE] } else { Vec::new() };
    out.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
    out
}

#[test]
fn reads_the_listing_wsl_prints() {
    assert_eq!(decode(&utf16("Ubuntu\r\nDebian\r\n", false)), "Ubuntu\nDebian\n");
    assert_eq!(decode(&utf16("Ubuntu\r\n", true)), "Ubuntu\n");
}

#[test]
fn reads_a_linux_programs_output_as_utf8() {
    assert_eq!(decode("MemTotal: 1 kB\nΑθήνα 東京\n".as_bytes()), "MemTotal: 1 kB\nΑθήνα 東京\n");
    assert_eq!(decode(b""), "");
    assert_eq!(decode(b"x"), "x");
}

#[test]
fn reads_names_that_are_not_ascii() {
    assert_eq!(distros(&decode(&utf16("Ubuntu-Δοκιμή\r\n", false))), ["Ubuntu-Δοκιμή"]);
}

#[test]
fn leaves_out_blanks_and_dockers_own() {
    let listing = decode(&utf16("Ubuntu\r\n\r\ndocker-desktop\r\nDocker-Desktop-Data\r\nArch\r\n\0", false));
    assert_eq!(distros(&listing), ["Ubuntu", "Arch"]);
}

#[test]
fn keeps_a_distro_named_like_docker_but_not_it() {
    assert_eq!(distros("docker-desktop-dev\nmy-docker-desktop\n"), ["docker-desktop-dev", "my-docker-desktop"]);
}

#[cfg(windows)]
#[tokio::test]
async fn gives_up_on_a_command_past_its_deadline() {
    // No WSL here or a distribution that takes its time: either way an
    // error, never a hang.
    let started = std::time::Instant::now();
    let _ = run(&["-e", "sleep", "30"], Duration::from_millis(300)).await;
    assert!(started.elapsed() < Duration::from_secs(5));
}
